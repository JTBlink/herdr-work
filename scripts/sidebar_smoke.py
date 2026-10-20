"""Exercise source Sidebar takeover in an isolated Herdr session.

Link this repository only in the disposable session first. The test creates
one workspace containing three working processes, checks real takeover and
exact restoration, then closes only that workspace. Never pass a user session.
"""

import argparse
from pathlib import Path
import json, os, socket, subprocess, time, shlex

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--socket",
    required=True,
    help="Explicit socket of an isolated, disposable Herdr test session",
)
parser.add_argument("--binary", required=True)
parser.add_argument("--independent-side", choices=["left", "right"])
parser.add_argument("--lifecycle", action="store_true")
parser.add_argument(
    "--activation-binary",
    help="Optional second installation used to verify focus/reuse",
)
parser.add_argument(
    "--output", required=True, help="Directory for temporary fixture evidence"
)
args = parser.parse_args()
SOCKET = Path(args.socket).resolve()
if str(SOCKET) == os.environ.get("HERDR_SOCKET_PATH"):
    raise SystemExit(
        "Refusing the inherited user session; choose an isolated test session"
    )
BINARY = Path(args.binary).resolve()
BASE = Path(args.output).resolve()
BASE.mkdir(parents=True)
REPO = BASE / "repo"
REPO.mkdir(exist_ok=True)
PROFILE = BASE / "ui-profile"
(PROFILE / "sidebar").mkdir(parents=True)
(PROFILE / "sidebar/state.json").write_text(
    json.dumps(
        {
            "merged": True,
            "sidebar_width": 28,
            "auto_open": False,
            "font_prompt": True,
            "preview_placement": "replace",
        }
    )
)

for name in ["a-file.txt", "b-file.txt"]:
    (REPO / name).write_text(
        "".join(f"{name} fixture line {i:03d}\n" for i in range(1, 151))
    )
subprocess.run(["git", "init", "-q", str(REPO)], check=True)
worker = BASE / "working.py"
worker.write_text(
    'import os,sys,time\nos.chdir(sys.argv[1])\nprint("WORKING "+sys.argv[2]+" PID "+str(os.getpid()),flush=True)\nwhile True: time.sleep(1)\n'
)


def rpc(method, params=None):
    with socket.socket(socket.AF_UNIX) as s:
        s.settimeout(15)
        s.connect(str(SOCKET))
        s.sendall(
            (
                json.dumps({"id": "fixture", "method": method, "params": params or {}})
                + "\n"
            ).encode()
        )
        result = json.loads(s.makefile().readline())
        if "error" in result:
            raise RuntimeError((method, result["error"]))
        return result["result"]


def panes():
    return rpc("pane.list")["panes"]


def read(pane):
    return rpc("pane.read", {"pane_id": pane, "source": "visible"})["read"]["text"]


def wait(predicate, label, timeout=25):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = predicate()
        if value:
            return value
        time.sleep(0.1)
    raise AssertionError(label)


def send(pane, text="", keys=None):
    return rpc("pane.send_input", {"pane_id": pane, "text": text, "keys": keys or []})


def layout(pane):
    return rpc("pane.layout", {"pane_id": pane})["layout"]


def process(pane):
    return rpc("pane.process_info", {"pane_id": pane})["process_info"]


def shape(value):
    return {
        "panes": sorted([(p["pane_id"], p["rect"]) for p in value["panes"]]),
        "splits": sorted(
            [
                (p["direction"], round(p["ratio"], 6), p["rect"])
                for p in value["splits"]
            ],
            key=str,
        ),
    }


workspace = None
try:
    created = rpc(
        "workspace.create",
        {"cwd": str(REPO), "label": "Source Sidebar native test", "focus": False},
    )
    original = created["root_pane"]["pane_id"]
    workspace = created["workspace"]["workspace_id"]
    send(
        original,
        "python3 -u "
        + shlex.quote(str(worker))
        + " "
        + shlex.quote(str(REPO))
        + " one",
        ["Enter"],
    )
    wait(lambda: "WORKING one" in read(original), "original working process")
    independent = None
    if args.independent_side:
        independent = rpc(
            "pane.split",
            {
                "target_pane_id": original,
                "direction": "right",
                "ratio": 0.15 if args.independent_side == "left" else 0.85,
                "cwd": str(REPO),
                "focus": False,
            },
        )["pane"]["pane_id"]
        if args.independent_side == "left":
            rpc(
                "pane.swap", {"source_pane_id": original, "target_pane_id": independent}
            )
        send(
            independent,
            "python3 -u "
            + shlex.quote(str(worker))
            + " "
            + shlex.quote(str(REPO))
            + " independent",
            ["Enter"],
        )
        wait(lambda: "WORKING independent" in read(independent), "independent fixture")
        rpc(
            "pane.report_metadata",
            {
                "pane_id": independent,
                "source": "herdr-sidebar-explorer",
                "tokens": {"herdr-sidebar-explorer": "1"},
            },
        )
        independent_process = process(independent)
    second = rpc(
        "pane.split",
        {
            "target_pane_id": original,
            "direction": "right",
            "ratio": 0.57,
            "cwd": str(REPO),
            "focus": False,
        },
    )["pane"]["pane_id"]
    third = rpc(
        "pane.split",
        {
            "target_pane_id": second,
            "direction": "down",
            "ratio": 0.41,
            "cwd": str(REPO),
            "focus": False,
        },
    )["pane"]["pane_id"]
    for pane, label in [(second, "two"), (third, "three")]:
        send(
            pane,
            "python3 -u "
            + shlex.quote(str(worker))
            + " "
            + shlex.quote(str(REPO))
            + " "
            + label,
            ["Enter"],
        )
        wait(lambda p=pane, l=label: "WORKING " + l in read(p), "working " + label)
    originals = [original, second, third]
    independent_rect = next((p["rect"] for p in layout(original)["panes"] if p["pane_id"] == independent), None)
    context = {
        "workspace_id": workspace,
        "focused_pane_id": original,
        "focused_pane_cwd": str(REPO),
    }
    env = dict(
        os.environ,
        HERDR_ENV="1",
        HERDR_SOCKET_PATH=str(SOCKET),
        HERDR_PLUGIN_CONTEXT_JSON=json.dumps(context),
        HERDR_PANE_ID=original,
    )
    env.pop("HERDR_PLUGIN_ID", None)
    result = subprocess.run(
        [str(BINARY), "--home", str(PROFILE), "ui", str(REPO)],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, result.stderr
    owner = json.loads(result.stdout)["pane_id"]
    print("Opened source Sidebar:", owner, flush=True)
    wait(
        lambda: any(
            p["pane_id"] == owner
            and p.get("tokens", {}).get("herdr-work-ready") == "true"
            for p in panes()
        ),
        "owned Sidebar readiness",
    )
    wait(lambda: "repo" in read(owner).casefold(), "source Explorer header")

    def title():
        return next(p.get("label") for p in panes() if p["pane_id"] == owner)

    wait(lambda: title() == "HerdrWork · Explorer", "product Explorer title")
    send(owner, keys=["2"])
    wait(lambda: title() == "HerdrWork · Search", "product Search title")
    send(owner, keys=["1"])
    wait(lambda: title() == "HerdrWork · Explorer", "Explorer before settings")
    send(owner, "s")
    wait(lambda: "HerdrWork Settings" in read(owner), "product settings title")
    send(owner, keys=["Escape"])
    send(owner, keys=["3"])
    wait(lambda: title() == "HerdrWork · Git", "product Git title")
    send(owner, keys=["1"])
    wait(lambda: title() == "HerdrWork · Explorer", "return to Explorer title")

    reused = subprocess.run(
        [
            str(Path(args.activation_binary).resolve())
            if args.activation_binary
            else str(BINARY),
            "--home",
            str(PROFILE),
            "activate",
            "show-explorer",
            "--root",
            str(REPO),
        ],
        env=env,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert reused.returncode == 0, reused.stderr
    receipt = json.loads(reused.stdout)
    assert receipt["action"] == "focused" and receipt["pane_id"] == owner, receipt

    if args.lifecycle:
        extra = rpc(
            "tab.create", {"workspace_id": workspace, "cwd": str(REPO), "focus": False}
        )
        extra_tab = extra["tab"]["tab_id"]
        extra_root = extra["root_pane"]["pane_id"]
        extra_env = dict(
            env,
            HERDR_PANE_ID=extra_root,
            HERDR_TAB_ID=extra_tab,
            HERDR_PLUGIN_CONTEXT_JSON=json.dumps(
                {
                    "focused_pane_id": extra_root,
                    "focused_pane_cwd": str(REPO),
                    "workspace_id": workspace,
                }
            ),
        )

        def launch(*command):
            r = subprocess.run(
                [str(BINARY), "--home", str(PROFILE), *command],
                env=extra_env,
                capture_output=True,
                text=True,
                timeout=30,
            )
            assert r.returncode == 0, r.stderr
            return json.loads(r.stdout)

        def preferences(**changed):
            path = PROFILE / "sidebar/state.json"
            state = json.loads(path.read_text())
            state.update(changed)
            pending = path.with_suffix(".test-pending")
            pending.write_text(json.dumps(state))
            pending.replace(path)

        assert launch("sidebar-ensure")["action"] == "disabled"
        preferences(auto_open=True)
        rpc("pane.focus", {"pane_id": owner})
        focus_before = [p["pane_id"] for p in panes() if p["focused"]]
        second_owner = launch("sidebar-ensure")["pane_id"]
        assert second_owner != owner
        wait(
            lambda: any(
                p["pane_id"] == second_owner
                and p.get("tokens", {}).get("herdr-work-ready") == "true"
                for p in panes()
            ),
            "second tab Sidebar ready",
        )
        assert next(p["pane_id"] for p in panes() if p["focused"]) == owner, (
            "quiet ensure stole focus", focus_before, [(p["pane_id"],p.get("focused")) for p in panes()]
        )
        preferences(strict_toggle=True)
        assert launch("activate", "toggle")["action"] == "closing"
        wait(
            lambda: all(p["pane_id"] != second_owner for p in panes()),
            "strict toggle close",
        )
        assert launch("sidebar-ensure")["action"] == "hidden"
        preferences(strict_toggle=False, focus_on_open=False)
        second_owner = launch("activate", "toggle")["pane_id"]
        wait(
            lambda: any(
                p["pane_id"] == second_owner
                and p.get("tokens", {}).get("herdr-work-ready") == "true"
                for p in panes()
            ),
            "toggle reopens",
        )
        assert next(p["pane_id"] for p in panes() if p["focused"]) == owner, (
            "focus-on-open off stole focus"
        )
        send(second_owner, keys=["2"])
        wait(lambda: next(p.get("label") for p in panes() if p["pane_id"] == second_owner) == "HerdrWork · Search", "search before focus-only toggle")
        rpc("pane.focus", {"pane_id": owner})
        assert launch("activate", "toggle")["action"] == "focused"
        time.sleep(.6)
        assert next(p.get("label") for p in panes() if p["pane_id"] == second_owner) == "HerdrWork · Search", "focus-only toggle reset activity"
        send(second_owner, keys=["1"])
        wait(lambda: next(p.get("label") for p in panes() if p["pane_id"] == second_owner) == "HerdrWork · Explorer", "return after focus-only toggle")
        send(second_owner, "b")
        wait(
            lambda: all(p["pane_id"] != second_owner for p in panes()), "source b hide"
        )
        assert launch("sidebar-ensure")["action"] == "hidden"
        preferences(focus_on_open=True)
        second_owner = launch("activate", "toggle")["pane_id"]
        wait(
            lambda: any(p["pane_id"] == second_owner and p["focused"] for p in panes()),
            "focus-on-open on",
        )
        preferences(auto_open=False)
        rpc("tab.close", {"tab_id": extra_tab})
        print(
            "Shared-profile tabs, auto-open, strict toggle, focus preference and hide/snooze passed.",
            flush=True,
        )

    rpc("pane.focus", {"pane_id": owner})
    time.sleep(0.7)
    before = layout(owner)
    if independent:
        assert next(p["rect"] for p in before["panes"] if p["pane_id"] == independent) == independent_rect, "opening resized the independent plugin"
    processes = {p: process(p) for p in originals}
    assert len(before["panes"]) == 4 + bool(independent), before
    (BASE / "before.json").write_text(
        json.dumps({"layout": before, "processes": processes}, indent=2)
    )
    assert next(p for p in before["panes"] if p["pane_id"] == owner)["rect"][
        "width"
    ] in (28, 30), before
    # Original Sidebar's Quick Open: type into its own input handler, then open.
    send(owner, keys=["ctrl+p"])
    wait(lambda: "Quick Open" in read(owner), "original Quick Open")
    send(owner, "a-file")
    send(owner, keys=["Enter"])

    def active_viewer():
        return next(
            (
                p
                for p in panes()
                if p["pane_id"] != owner
                and p.get("tokens", {}).get("herdr-work-preview-inline") == "1"
                and p["tab_id"] == before["tab_id"]
            ),
            None,
        )

    try:
        viewer = wait(active_viewer, "actual takeover viewer")["pane_id"]
    except AssertionError:
        (BASE / "failed-screen.txt").write_text(read(owner))
        (BASE / "failed-layout.json").write_text(json.dumps(layout(owner),indent=2))
        raise
    wait(lambda: "a-file.txt fixture line 001" in read(viewer), "viewer document")
    during = layout(owner)
    assert {p["pane_id"] for p in during["panes"]} == {owner, viewer} | (
        {independent} if independent else set()
    ), during
    if independent:
        assert process(independent) == independent_process
        assert next(
            p["rect"] for p in during["panes"] if p["pane_id"] == independent
        ) == next(p["rect"] for p in before["panes"] if p["pane_id"] == independent)
    working_width = during["area"]["width"] - (next(
        p["rect"]["width"] for p in during["panes"] if p["pane_id"] == independent
    ) if independent else 0)
    min_width = max(28, int(working_width * 0.1 + 0.5))
    # Attached Herdr clients reserve two columns for pane chrome.
    assert next(p for p in during["panes"] if p["pane_id"] == owner)["rect"][
        "width"
    ] in (min_width, min_width + 2), during

    parked = {p["pane_id"]: p["tab_id"] for p in panes() if p["pane_id"] in originals}
    assert set(parked) == set(originals) and all(
        tab != before["tab_id"] for tab in parked.values()
    ), parked
    for pane in originals:
        assert (
            process(pane)["foreground_processes"]
            == processes[pane]["foreground_processes"]
        ), (pane, "process changed while parked")
    print(
        "Takeover occupies real space; all three working processes parked and alive.",
        flush=True,
    )
    # Reuse viewer for a second file; never create another working-area pane.
    rpc("pane.focus", {"pane_id": owner})
    send(owner, keys=["ctrl+p"])
    wait(lambda: "Quick Open" in read(owner), "second Quick Open")
    send(owner, "b-file")
    send(owner, keys=["Enter"])
    wait(
        lambda: "b-file.txt fixture line 001" in read(viewer),
        "same viewer switches document",
    )
    assert active_viewer()["pane_id"] == viewer
    # The source editor must keep dirty text when close is cancelled.
    send(viewer, "e")
    wait(
        lambda: any(
            p["pane_id"] == viewer and "editor" in p.get("label", "") for p in panes()
        ),
        "source editor",
    )
    send(viewer, "edited-")
    send(viewer, keys=["ctrl+q"])
    wait(lambda: "Unsaved changes" in read(viewer), "unsaved close guard")
    send(viewer, keys=["Escape"])
    assert not (REPO / "b-file.txt").read_text().startswith("edited-")
    assert viewer in {p["pane_id"] for p in panes()}
    send(viewer, keys=["ctrl+s"])
    wait(
        lambda: (REPO / "b-file.txt").read_text().startswith("edited-"),
        "source editor save",
    )
    send(viewer, keys=["Escape"])
    wait(
        lambda: any(
            p["pane_id"] == viewer and "preview" in p.get("label", "") for p in panes()
        ),
        "leave clean editor",
    )
    print("Source editor dirty close/cancel/save/preview passed.", flush=True)
    # Close the running viewer itself: this runs source restore before pane close.
    send(viewer, keys=["q"])
    wait(
        lambda: shape(layout(owner)) == shape(before),
        "exact original split tree restored",
    )
    after = layout(owner)
    if independent:
        assert process(independent) == independent_process
    for pane in originals:
        assert (
            process(pane)["foreground_processes"]
            == processes[pane]["foreground_processes"]
        ), (pane, "original process identity changed")
    assert after["focused_pane_id"] == owner, after
    assert not list((PROFILE / "sidebar/takeover").glob("*.json"))
    print(
        "Restored original pane IDs, process IDs, split directions/ratios/rectangles and Sidebar focus.",
        flush=True,
    )
    (BASE / "after.json").write_text(
        json.dumps(
            {"layout": after, "processes": {p: process(p) for p in originals}}, indent=2
        )
    )
    # A host-side close bypasses the viewer's own quit handler. The source
    # recovery journal must still bring every working pane back.
    send(owner, keys=["ctrl+p"])
    wait(lambda: "Quick Open" in read(owner), "recovery Quick Open")
    send(owner, "a-file")
    send(owner, keys=["Enter"])
    interrupted = wait(active_viewer, "second takeover viewer")["pane_id"]
    wait(
        lambda: "a-file.txt fixture line 001" in read(interrupted), "recovery document"
    )
    rpc("pane.close", {"pane_id": interrupted})
    wait(lambda: shape(layout(owner)) == shape(before), "background journal recovery")
    for pane in originals:
        assert (
            process(pane)["foreground_processes"]
            == processes[pane]["foreground_processes"]
        )
    assert not list((PROFILE / "sidebar/takeover").glob("*.json"))
    print("Host-side viewer close recovered the exact original layout.", flush=True)
    (BASE / "native-result.json").write_text(
        json.dumps(
            {
                "passed": True,
                "sidebar": owner,
                "viewer": viewer,
                "originals": originals,
                "exact_layout": True,
                "processes_preserved": True,
                "interrupted_viewer_recovered": True,
            },
            indent=2,
        )
    )
finally:
    if workspace is not None:
        rpc("workspace.close", {"workspace_id": workspace})
