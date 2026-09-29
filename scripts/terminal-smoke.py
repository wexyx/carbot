"""Exercise the real TTY editor, colors, secret masking and terminal restoration."""
import fcntl
import json
import os
import pathlib
import pty
import select
import shutil
import signal
import struct
import tempfile
import termios
import time

root = pathlib.Path(__file__).resolve().parent.parent
directory = tempfile.mkdtemp(prefix="carbot-terminal-")
pid, master = pty.fork()
if pid == 0:
    os.chdir(directory)
    os.execve(str(root / "target/debug/agent-node"), ["agent-node", "--cli"], {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"), "TERM": "xterm-256color",
        "ADMIN_AGENT_PROVIDER": "mock", "CARBOT_DATA_DIR": directory,
    })

fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 28, 110, 0, 0))
output = bytearray()
position = 0

def wait_for(text):
    global position
    expected = text.encode()
    deadline = time.monotonic() + 6
    while time.monotonic() < deadline:
        if expected in output[position:]:
            position = len(output)
            return
        if select.select([master], [], [], 0.1)[0]:
            output.extend(os.read(master, 65536))
    raise AssertionError("Terminal did not show " + text)

def send(text):
    os.write(master, text.encode())

try:
    wait_for("CARBOT")
    assert output.index("直接输入任务".encode()) < output.index(b"CARBOT"), "session metadata must follow the chat/input area"
    assert b"\x1b[?1000h" not in output, "native selection must be enabled by default"
    assert b"\x1b[38;5;14m" in output or b"\x1b[36m" in output, "missing cyan theme"
    send("/admin-c\t")
    wait_for("/admin-config ")
    send("\r")
    wait_for("配置 1/6")
    for value, prompt in [("carbot", "厂商"), ("ollama", "模型名称"), ("fixture", "接口地址"), ("-", "协议"), ("-", "API Key")]:
        send(value + "\r")
        wait_for(prompt)
    send("tty-secret-fixture")
    wait_for("******************")
    assert b"tty-secret-fixture" not in output, "secret appeared in terminal"
    send("\r")
    wait_for("配置已保存并生效")
    settings = json.loads((pathlib.Path(directory) / "default-agent.json").read_text())
    assert settings["MODEL_API_KEY"] == "tty-secret-fixture"
    send("\x1b[A")
    wait_for("/admin-config")
    send("\x15/he\t\r")
    wait_for("/help network")
    time.sleep(0.1)
    send("\x1b[200~你好\n第二行\x1b[201~")
    wait_for("第二行")
    send("\x1b")
    time.sleep(0.15)
    send("\x04")
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            if select.select([master], [], [], 0.1)[0]:
                output.extend(os.read(master, 65536))
        except OSError:
            break
    _, status = os.waitpid(pid, 0)
    pid = None
    assert os.waitstatus_to_exitcode(status) == 0
    assert b"\x1b[?1049h" not in output, "inline REPL must preserve native scrollback"
    print("TTY passed: colors, completion, history, multiline paste, masked configuration, clean exit")
finally:
    if pid is not None:
        try:
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
        except ProcessLookupError:
            pass
    os.close(master)
    shutil.rmtree(directory)
