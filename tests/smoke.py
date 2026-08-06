"""End-to-end smoke test for ephe.

Runs the binary, exercises both peers, then sends Ctrl+C / process terminate.
"""
import asyncio
import json
import os
import re
import socket
import subprocess
import sys
import threading
import time

import websockets


def drain(stream, label):
    while True:
        line = stream.readline()
        if not line:
            return
        print(f"[{label}] {line.decode('utf-8', errors='replace').rstrip()}", flush=True)


async def main():
    env = os.environ.copy()
    env["EPHE_NO_BROWSER"] = "1"
    proc = subprocess.Popen(
        [r"C:\Projects\Ephe\target\release\ephe.exe"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    threading.Thread(target=drain, args=(proc.stderr, "ERR"), daemon=True).start()
    try:
        deadline = time.time() + 5
        text = ""
        while time.time() < deadline:
            line = proc.stdout.readline()
            if not line:
                await asyncio.sleep(0.05)
                continue
            text += line.decode("utf-8", errors="replace")
            if "mobile :" in text:
                break
        m = re.search(r"localhost:(\d+)", text)
        port = int(m.group(1))
        m = re.search(r"token=([0-9a-f]+)", text)
        token = m.group(1)
        m = re.search(r"mobile : http://([\d.]+):", text)
        lan_ip = m.group(1)
        local_ip = lan_ip
        print(f"port={port} token={token} lan_ip={lan_ip}", flush=True)

        async with websockets.connect(f"ws://{lan_ip}:{port}/ws?token={token}", local_addr=(local_ip, 0)) as mobile:
            msg = await mobile.recv()
            assert json.loads(msg)["type"] == "init"
            print("MOBILE init ok", flush=True)
            async with websockets.connect(f"ws://localhost:{port}/ws?token={token}") as desktop:
                msg = await asyncio.wait_for(desktop.recv(), timeout=3)
                assert json.loads(msg)["type"] == "init"
                print("DESKTOP init ok", flush=True)
                mmsg = await asyncio.wait_for(mobile.recv(), timeout=3)
                assert json.loads(mmsg)["type"] == "peer_joined"
                print("MOBILE peer_joined ok", flush=True)
                await desktop.send(json.dumps({"type": "text", "text": "from desktop"}))
                txt = await asyncio.wait_for(mobile.recv(), timeout=3)
                assert json.loads(txt) == {"type": "text", "text": "from desktop"}
                print("sync desktop->mobile ok", flush=True)
                await mobile.send(json.dumps({"type": "text", "text": "from mobile"}))
                txt = await asyncio.wait_for(desktop.recv(), timeout=3)
                assert json.loads(txt) == {"type": "text", "text": "from mobile"}
                print("sync mobile->desktop ok", flush=True)
            pl = await asyncio.wait_for(mobile.recv(), timeout=3)
            assert json.loads(pl)["type"] == "peer_left"
            print("MOBILE peer_left ok", flush=True)

        # mobile closed. with reset semantics the binary should still be running.
        time.sleep(0.3)
        if proc.poll() is not None:
            raise AssertionError(f"binary exited prematurely with code {proc.returncode}")
        print("binary still running after both peers left", flush=True)

        # hit /reset to rotate the token and confirm a new mobile can join
        import urllib.request
        req = urllib.request.Request(f"http://localhost:{port}/reset", method="POST")
        with urllib.request.urlopen(req, timeout=3) as r:
            data = json.loads(r.read())
        new_url = data["url"]
        new_token = re.search(r"token=([0-9a-f]+)", new_url).group(1)
        assert new_token != token
        print(f"reset ok, new token={new_token[:8]}\u2026", flush=True)

        # new mobile joins with the rotated token
        async with websockets.connect(f"ws://{lan_ip}:{port}/ws?token={new_token}", local_addr=(local_ip, 0)) as mobile2:
            msg = await mobile2.recv()
            j = json.loads(msg)
            assert j["type"] == "init"
            assert j["text"] == "", "fresh session must start empty"
            print("new MOBILE joined with rotated token, text empty", flush=True)

        proc.terminate()
        try:
            proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=2)
        print(f"PROC terminated (rc={proc.returncode})", flush=True)
        print("OK", flush=True)
        return 0
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                proc.kill()


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))