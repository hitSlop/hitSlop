#!/usr/bin/env python3
"""Manual, isolated release stress experiment; never edits the supplied corpus.

Run against an extracted release engine and its original compatibility corpus:
  python3 scripts/dev/stress-release.py --engine /path/tiny-engine \
    --corpus /path/corpus --output /tmp/stress.json

This records observed failures rather than treating known findings as passing CI.
All destructive inputs, interrupted writers and mutations use disposable copies.
The native WebKit host, macOS services and physical power failure are out of scope.
"""

import argparse
import base64
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import queue
import random
import resource
import shutil
import signal
import sqlite3
import subprocess
import tempfile
import threading
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_process(command, payload=None, timeout=8, env=None):
    # A bad file must not exhaust the review machine or leave evaluator descendants.
    if os.name == "posix" and shutil.which("prlimit"):
        command = ["prlimit", "--as=2147483648", "--cpu=15", "--", *command]
    started = time.monotonic()
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True, env=env)
    timed_out = False
    try:
        stdout, stderr = process.communicate(payload, timeout=timeout)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
    result = {"exit": process.returncode, "timeout": timed_out,
              "seconds": round(time.monotonic() - started, 4),
              "stderr": stderr.decode(errors="replace")[-1000:]}
    try:
        result["reply"] = json.loads(stdout)
    except (ValueError, UnicodeDecodeError):
        result["stdout"] = stdout[:500].decode(errors="replace")
    return result


class PreviewOwner:
    """The released preview adapter exercises the same production Rust owner/store.

    Its 64-request admission cap is NOT the native WebKit bridge's admission policy.
    """
    def __init__(self, engine, path, env):
        self.process = subprocess.Popen([engine, "--client-protocol", "1", "--preview-owner", str(path)],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.PIPE, start_new_session=True, env=env)
        self.events = queue.Queue()
        self.serial = 0
        self.peak_rss_kib = 0
        self.proc_pid = None
        def read():
            for line in self.process.stdout:
                self.events.put(json.loads(line))
            self.events.put({"type": "eof"})
        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()
        self.next_type("ready")

    def rss(self):
        try:
            # /proc may be mounted from an outer PID namespace. Popen's PID then
            # names an unrelated outer process; match both parent and NSpid first.
            if self.proc_pid is None:
                parent = int(Path("/proc/self/status").read_text().split("Pid:")[1].split()[0])
                for entry in Path("/proc").iterdir():
                    if not entry.name.isdigit():
                        continue
                    try:
                        status = (entry / "status").read_text()
                        ppid = int(status.split("PPid:")[1].split()[0])
                        inner = int(status.split("NSpid:")[1].splitlines()[0].split()[-1])
                        if ppid == parent and inner == self.process.pid:
                            self.proc_pid = entry.name
                            break
                    except (OSError, IndexError):
                        continue
            if self.proc_pid is None:
                return None
            text = Path(f"/proc/{self.proc_pid}/status").read_text()
            value = int(text.split("VmRSS:")[1].split()[0])
            self.peak_rss_kib = max(self.peak_rss_kib, value)
            return value
        except (OSError, IndexError):
            return None

    def next_type(self, kind, timeout=20, request_id=None):
        end = time.monotonic() + timeout
        while True:
            event = self.events.get(timeout=max(0.01, end - time.monotonic()))
            if event.get("type") in ("fatal", "eof"):
                raise RuntimeError(str(event))
            if event.get("type") == kind and (request_id is None or event.get("id") == request_id):
                return event
            if time.monotonic() > end:
                raise TimeoutError(kind)

    def call(self, request, timeout=20):
        self.serial += 1
        self.process.stdin.write(json.dumps({"type": "page", "id": self.serial, "request": request}).encode() + b"\n")
        self.process.stdin.flush()
        return json.loads(self.next_type("reply", timeout, self.serial)["reply"])

    def stop(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                os.killpg(self.process.pid, signal.SIGKILL)
                self.process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--mutations", type=int, default=256)
    parser.add_argument("--soak-updates", type=int, default=10000)
    parser.add_argument("--sections", default="compat,files,fuzz,evaluator,concurrency,recovery,crash,atomic,paths,capacity,captures,soak")
    args = parser.parse_args()
    engine = str(Path(args.engine).resolve())
    corpus = args.corpus.resolve()
    sections = args.sections.split(",")
    report = {"engineSha256": digest(Path(engine)), "seed": 20261016,
              "platform": os.uname().sysname + " " + os.uname().machine,
              "sections": {}, "scope": "released engine; no macOS/WebKit execution"}
    with tempfile.TemporaryDirectory(prefix="tinys-stress-") as temporary:
        root = Path(temporary)
        env = os.environ | {"TINYS_TEST_REGISTRY": str(root / "registry")}
        def invoke(request, timeout=8):
            return run_process([engine, "--client-protocol", "1"], json.dumps(request).encode(), timeout, env)
        def good(request, timeout=15):
            result = invoke(request, timeout)
            assert result.get("reply", {}).get("ok") is True, result
            return result["reply"]
        def document(name):
            path = root / (name + ".tiny")
            shutil.copyfile(corpus / "documents/conformance.tiny", path)
            return path
        def get(path):
            return good({"method": "get", "documentPath": str(path)})["state"]["value"]
        def save_report():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(report, indent=2) + "\n")
        def finish(name, result):
            report["sections"][name] = result
            save_report()
            print(name + ": " + json.dumps(result)[:1800], flush=True)

        report["coreBuildID"] = subprocess.check_output([engine, "--build-id"], text=True).strip()
        if "compat" in sections:
            manifest = json.loads((corpus / "release.json").read_text())
            for path, expected in manifest["files"].items():
                assert digest(corpus / path) == expected, path
            for path in (corpus / "templates").glob("*.tiny"):
                target = root / ("new-" + path.name)
                good({"method": "create", "from": str(path), "output": str(target)})
                assert get(target) == json.loads((corpus / "expected" / ("new-" + path.stem + ".json")).read_text())
            for source in (corpus / "documents").glob("*.tiny"):
                target = root / ("saved-" + source.name)
                shutil.copyfile(source, target)
                expected = json.loads((corpus / "expected" / (source.stem + ".json")).read_text())
                assert get(target) == expected["value"]
                for attachment in expected["attachments"]:
                    reply = good({"method": "attachments.read", "documentPath": str(target), "attachmentID": attachment["id"]})
                    data = base64.b64decode(reply["state"]["bytes"])
                    assert len(data) == attachment["byteLength"] and hashlib.sha256(data).hexdigest() == attachment["id"]
                scenario = json.loads((corpus / "scenarios" / (source.stem + ".json")).read_text())
                good({"method": "batch", "documentPath": str(target), "batch": {"intents": scenario["ops"]}})
                assert get(target) == scenario["value"]
            finish("compat", {"manifestFiles": len(manifest["files"]), "templates": 11, "documents": 4, "result": "passed"})

        if "files" in sections:
            outcomes = {}
            for name, sql in {
                "ordinary-wrong-table": "CREATE TABLE app(id INTEGER,package_format INTEGER,runtime_abi INTEGER);INSERT INTO app VALUES(1,1,1);",
                "recursive-app-view": "CREATE VIEW app AS WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n) SELECT 1 id,1 package_format,(SELECT max(x) FROM n) runtime_abi;",
            }.items():
                path = root / (name + ".tiny")
                with sqlite3.connect(path) as db:
                    db.executescript("PRAGMA application_id=1213418576;PRAGMA user_version=1;" + sql)
                before = digest(path)
                outcomes[name] = invoke({"method": "inspect", "file": str(path)}, timeout=2)
                outcomes[name]["fileBytes"] = path.stat().st_size
                outcomes[name]["unchanged"] = digest(path) == before
            for name, sql in {"future-storage": "PRAGMA user_version=2;",
                              "empty-history": "DELETE FROM history;",
                              "bad-checkpoint": "UPDATE history SET bytes=x'00' WHERE seq=1;"}.items():
                path = document(name)
                with sqlite3.connect(path) as db:
                    db.executescript(sql)
                before = digest(path)
                outcomes[name] = invoke({"method": "get", "documentPath": str(path)})
                outcomes[name]["unchanged"] = digest(path) == before
            finish("files", outcomes)

        if "fuzz" in sections:
            source = (corpus / "documents/conformance.tiny").read_bytes()
            rng = random.Random(report["seed"])
            cases = []
            for i in range(args.mutations):
                path = root / f"mutation-{i}.tiny"
                if i % 2:
                    path.write_bytes(source)
                    with sqlite3.connect(path) as db:
                        data = bytearray(db.execute("SELECT bytes FROM history ORDER BY seq LIMIT 1").fetchone()[0])
                        for _ in range(1 + i % 8):
                            data[rng.randrange(len(data))] ^= rng.randrange(1, 256)
                        db.execute("UPDATE history SET bytes=? WHERE seq=(SELECT min(seq) FROM history)", (data,))
                else:
                    data = bytearray(source)
                    if i % 8 == 0:
                        data = data[:rng.randrange(0, len(data))]
                    else:
                        for _ in range(1 + i % 16):
                            data[rng.randrange(len(data))] ^= rng.randrange(1, 256)
                    path.write_bytes(data)
                cases.append((i, path, digest(path)))
            def mutate(item):
                i, path, before = item
                result = invoke({"method": "get", "documentPath": str(path)}, timeout=3)
                return {"case": i, "unchanged": before == digest(path), **result}
            with ThreadPoolExecutor(max_workers=4) as pool:
                outcomes = list(pool.map(mutate, cases))
            counts = Counter("timeout" if x["timeout"] else "signal" if x["exit"] < 0 else
                             "accepted" if x.get("reply", {}).get("ok") else "refused" for x in outcomes)
            anomalies = [x for x in outcomes if x["timeout"] or x["exit"] < 0 or not x["unchanged"]]
            finish("fuzz", {"cases": len(cases), "counts": counts, "anomalies": anomalies})

        if "evaluator" in sections:
            outcomes = {}
            programs = {
                "no-host-io": "globalThis.__tinyDescribe=()=>JSON.stringify({std:typeof std,os:typeof os,fetch:typeof fetch,process:typeof process,require:typeof require});",
                "infinite-loop": "while(true){}",
                "stack-overflow": "function f(){return f()}f();",
                "heap-exhaustion": "let a=[];while(true){a.push(new Uint8Array(1024*1024))}",
                "oversized-result": "globalThis.__tinyDescribe=()=> 'x'.repeat(4194305);",
                "prototype-tamper": "Object.prototype.polluted=true;globalThis.__tinyDescribe=()=>JSON.stringify({ok:true});",
                "fresh-realm": "globalThis.__tinyDescribe=()=>JSON.stringify({polluted:Object.prototype.polluted===true});",
            }
            for name, bundle in programs.items():
                job = {"runtimeABI": 1, "mode": "definition", "bundle": bundle, "request": "{}"}
                outcomes[name] = run_process([engine, "--evaluate-command"], json.dumps(job).encode(), 6, env)
            finish("evaluator", outcomes)

        if "concurrency" in sections:
            path = document("concurrent")
            before = get(path)["hits"]
            def increment(_):
                return invoke({"method": "batch", "documentPath": str(path),
                               "batch": {"intents": [{"type": "increment", "path": ["hits"], "by": 1}]}})
            with ThreadPoolExecutor(max_workers=12) as pool:
                outcomes = list(pool.map(increment, range(240)))
            acknowledged = sum(x.get("reply", {}).get("ok") is True for x in outcomes)
            final = get(path)["hits"]
            finish("concurrency", {"requests": len(outcomes), "acknowledged": acknowledged,
                                   "delta": final - before, "acknowledgedExactlyOnce": final - before == acknowledged,
                                   "failures": dict(Counter(x.get("reply", {}).get("code", str(x["exit"])) for x in outcomes if not x.get("reply", {}).get("ok")))})

        if "recovery" in sections:
            path = document("busy-and-move")
            owner = PreviewOwner(engine, path, env)
            try:
                with sqlite3.connect(path, timeout=1) as lock:
                    lock.execute("BEGIN EXCLUSIVE")
                    edited = owner.call({"method": "apply", "batch": {"intents": [{"type": "set", "path": ["title"], "value": "retained through busy"}]}})
                    busy = owner.call({"method": "flush"})
                    lock.rollback()
                retried = owner.call({"method": "flush"})
                moved = path.with_name("moved.tiny")
                path.rename(moved)
                owner.call({"method": "apply", "batch": {"intents": [{"type": "set", "path": ["title"], "value": "retained through move"}]}})
                missing = owner.call({"method": "flush"})
                moved.rename(path)
                restored = owner.call({"method": "flush"})
                finish("recovery", {"edit": edited, "busy": busy, "retry": retried, "moved": missing, "restored": restored})
            finally:
                owner.stop()
            assert get(path)["title"] == "retained through move"

        if "crash" in sections:
            baseline = get(corpus / "documents/conformance.tiny")
            outcomes = Counter()
            for i in range(40):
                path = document("crash-" + str(i))
                request = {"method": "batch", "documentPath": str(path), "batch": {"intents": [
                    {"type": "set", "path": ["title"], "value": "atomic new title"},
                    {"type": "set", "path": ["memo"], "value": "x" * 1000000},
                ]}}
                process = subprocess.Popen([engine, "--client-protocol", "1"], stdin=subprocess.PIPE,
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                           start_new_session=True, env=env)
                process.stdin.write(json.dumps(request).encode())
                process.stdin.close()
                time.sleep([0, 0.001, 0.003, 0.01, 0.03][i % 5])
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                try:
                    value = get(path)
                    state = "old" if value["title"] == baseline["title"] and value["memo"] == baseline["memo"] else \
                        "new" if value["title"] == "atomic new title" and value["memo"] == "x" * 1000000 else "partial"
                    outcomes[state] += 1
                except AssertionError:
                    outcomes["unreadable"] += 1
            # The large-input trials above can all die before a write begins. Also
            # vary the timing of a small two-field transaction to observe both sides
            # of publication; process death alone is not physical power failure.
            small = []
            for i, delay in enumerate([0.0001, 0.0005, 0.001, 0.002, 0.003, 0.005,
                                       0.008, 0.01, 0.02, 0.04, 0.08, 0.12] * 3):
                path = document("small-crash-" + str(i))
                request = {"method": "batch", "documentPath": str(path), "batch": {"intents": [
                    {"type": "set", "path": ["label"], "value": "after crash"},
                    {"type": "increment", "path": ["hits"], "by": 7},
                ]}}
                process = subprocess.Popen([engine, "--client-protocol", "1"], stdin=subprocess.PIPE,
                                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                           start_new_session=True, env=env)
                process.stdin.write(json.dumps(request).encode())
                process.stdin.close()
                time.sleep(delay)
                killed = process.poll() is None
                if killed:
                    os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                value = get(path)
                state = "old" if value["label"] == baseline["label"] and value["hits"] == baseline["hits"] else \
                    "new" if value["label"] == "after crash" and value["hits"] == baseline["hits"] + 7 else "partial"
                small.append({"delay": delay, "killed": killed, "exit": process.returncode, "outcome": state})
            finish("crash", {"largeInputTrials": 40, "largeInputOutcomes": outcomes, "smallInputTrials": small})

        if "atomic" in sections:
            path = document("refused-batches")
            before = get(path)
            invalid = [
                {"type": "set", "path": ["count"], "value": "bad"},
                {"type": "set", "path": ["ceiling"], "value": 1000000000},
                {"type": "set", "path": ["missing"], "value": 1},
                {"type": "set", "path": ["__proto__", "polluted"], "value": 1},
                {"type": "insert", "path": ["rows"], "id": "../evil", "value": {"text": "bad", "done": False}},
                {"type": "set", "path": ["title"], "value": None},
                {"type": "increment", "path": ["hits"], "by": 1.5},
                {"type": "set", "path": ["stages", "__proto__"], "value": "todo"},
            ]
            results = []
            for i in range(96):
                bad = invalid[i % len(invalid)]
                valid = {"type": "set", "path": ["label"], "value": "must not leak " + str(i)}
                ops = [valid, bad] if i % 2 else [bad, valid]
                response = invoke({"method": "batch", "documentPath": str(path), "batch": {"intents": ops}})
                results.append({"refused": response.get("reply", {}).get("ok") is False,
                                "atomic": get(path) == before})
            finish("atomic", {"cases": len(results), "allRefused": all(x["refused"] for x in results),
                              "allAtomic": all(x["atomic"] for x in results)})

        if "paths" in sections:
            source = document("path-source")
            linked = root / "linked.tiny"
            linked.symlink_to(source)
            hard = root / "hard.tiny"
            os.link(source, hard)
            checks = {
                "symlink-leaf-get": {"method": "get", "documentPath": str(linked)},
                "hardlink-write": {"method": "batch", "documentPath": str(hard), "batch": {"intents": [
                    {"type": "increment", "path": ["hits"], "by": 1},
                ]}},
            }
            results = {name: invoke(request) for name, request in checks.items()}
            hard.unlink()
            sentinel = root / "must-not-replace.tiny"
            sentinel.write_text("ORIGINAL SENTINEL")
            checks = {
                "create-must-not-overwrite": {"method": "create", "from": str(corpus / "templates/conformance.tiny"), "output": str(sentinel)},
                "copy-must-not-overwrite": {"method": "copy", "documentPath": str(source), "output": str(sentinel)},
                "export-must-not-overwrite": {"method": "export", "documentPath": str(source), "format": "png", "output": str(sentinel)},
            }
            results.update({name: invoke(request) for name, request in checks.items()})
            assert all(results[name].get("reply", {}).get("reason") == "exists" for name in checks), results
            finish("paths", {"checks": results, "destinationPreserved": sentinel.read_text() == "ORIGINAL SENTINEL"})

        if "capacity" in sections:
            path = document("capacity")
            owner = PreviewOwner(engine, path, env)
            rng = random.Random(report["seed"])
            samples, failure, restored = [], None, None
            try:
                for i in range(48):
                    text = base64.b64encode(rng.randbytes(786432)).decode()
                    row = "near-" + str(i)
                    accepted = owner.call({"method": "apply", "batch": {"intents": [
                        {"type": "insert", "path": ["rows"], "id": row, "value": {"text": text, "done": False}},
                    ]}}, timeout=45)
                    assert accepted.get("ok"), accepted
                    saved = owner.call({"method": "flush"}, timeout=45)
                    samples.append({"insertedMiB": i + 1, "saved": saved.get("ok") is True,
                                    "fileBytes": path.stat().st_size, "rssKiB": owner.rss()})
                    if not saved.get("ok"):
                        failure = saved
                        owner.call({"method": "apply", "batch": {"intents": [
                            {"type": "remove", "path": ["rows"], "id": row},
                        ]}}, timeout=45)
                        restored = owner.call({"method": "flush"}, timeout=45)
                        break
            finally:
                owner.stop()
            finish("capacity", {"samples": samples, "firstSaveFailure": failure, "saveAfterDelete": restored})

        if "captures" in sections:
            outputs = []
            for source in sorted((corpus / "documents").glob("*.tiny")):
                for extension in ["png", "pdf"]:
                    target = root / (source.stem + "." + extension)
                    result = invoke({"method": "export", "documentPath": str(source),
                                     "format": extension, "output": str(target)}, timeout=25)
                    outputs.append({"document": source.stem, "format": extension,
                                    "bytes": target.stat().st_size if target.exists() else 0, **result})
            finish("captures", outputs)

        if "soak" in sections:
            path = document("continuous-owner")
            owner = PreviewOwner(engine, path, env)
            latencies, samples = [], []
            rng = random.Random(report["seed"])
            started = time.monotonic()
            try:
                for i in range(args.soak_updates):
                    text = base64.b64encode(rng.randbytes(768)).decode()
                    t = time.monotonic()
                    reply = owner.call({"method": "apply", "batch": {"intents": [{"type": "set", "path": ["memo"], "value": text}]}})
                    assert reply.get("ok"), reply
                    latencies.append((time.monotonic() - t) * 1000)
                    if i % 1000 == 0:
                        flush = owner.call({"method": "flush"})
                        assert flush.get("ok"), flush
                        samples.append({"edits": i + 1, "rssKiB": owner.rss(), "fileBytes": path.stat().st_size})
                        print("soak progress: " + json.dumps(samples[-1]), flush=True)
                assert owner.call({"method": "flush"}).get("ok")
            finally:
                owner.stop()
            assert get(path)["memo"] == text
            times = sorted(latencies)
            finish("soak", {"edits": len(times), "seconds": round(time.monotonic() - started, 3),
                            "p50ms": round(times[len(times)//2], 3), "p99ms": round(times[int(len(times)*.99)], 3),
                            "maxms": round(max(times), 3), "peakSampledRssKiB": owner.peak_rss_kib,
                            "peakChildRssKiB": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
                            "samples": samples, "savedValueVerified": True})
    print("Evidence written to " + str(args.output), flush=True)


if __name__ == "__main__":
    main()
