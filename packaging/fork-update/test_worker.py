#!/usr/bin/env python3
"""Fast updater protocol/safety tests. No real containers, agents or builds."""
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


worker = load("fork_worker", HERE / "worker.py")
builder = load("fork_builder", HERE / "build.py")


class WorkerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="spotifast-worker-test-")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = worker.private_directory(self.base / "state")
        self.data = self.base / "data"

    def status(self, **values):
        return worker.publish(self.root, **values)

    def test_status_read_does_not_start_anything(self):
        missing = self.base / "absent"
        with patch.object(worker, "command") as called:
            self.assertEqual(worker.snapshot(missing)["phase"], "idle")
        called.assert_not_called()
        self.assertFalse(missing.exists())

    def test_rejects_invalid_release_before_starting_a_process(self):
        for version in ["../main", "1.2.3;touch bad", "--help", "main", ""]:
            with self.subTest(version=version), patch.object(worker, "command") as called:
                with self.assertRaises(RuntimeError):
                    worker.start(self.root, version, self.data)
                called.assert_not_called()

    def test_click_starts_one_detached_worker_with_argument_vector(self):
        calls = []
        def fake(arguments, **_):
            calls.append([str(value) for value in arguments])
            return SimpleNamespace(returncode=0, stdout="", stderr="")
        with patch.object(worker.shutil, "which", return_value="/usr/bin/tmux"), \
             patch.object(worker, "command", side_effect=fake):
            state = worker.start(self.root, "0.15.0", self.data)
        self.assertEqual(state["phase"], "starting")
        self.assertRegex(state["session"], r"^spotifast-update-[a-f0-9]{24}$")
        self.assertIn("new-session", calls[0])
        self.assertIn("worker", calls[0])
        self.assertIn("--state-dir", calls[0])
        self.assertTrue((self.root / "runs" / state["run_id"] / "tools/pi.ts").is_file())
        self.assertEqual(self.root.stat().st_mode & 0o077, 0)

    def test_second_click_reconnects_without_starting_duplicate(self):
        old = self.status(phase="building", version="0.15.0",
                          session="spotifast-update-" + "a" * 24)
        with patch.object(worker.shutil, "which", return_value="/usr/bin/tmux"), \
             patch.object(worker, "tmux_alive", return_value=True), \
             patch.object(worker, "command") as called:
            self.assertEqual(worker.start(self.root, "0.16.0", self.data), old)
        called.assert_not_called()

    def test_ready_same_version_does_not_rebuild(self):
        old = self.status(phase="ready", version="0.15.0")
        with patch.object(worker.shutil, "which", return_value="/usr/bin/tmux"), \
             patch.object(worker, "command") as called:
            self.assertEqual(worker.start(self.root, "0.15.0", self.data), old)
        called.assert_not_called()

    def test_failed_detach_is_visible_not_false_success(self):
        with patch.object(worker.shutil, "which", return_value="/usr/bin/tmux"), \
             patch.object(worker, "command", side_effect=RuntimeError("tmux refused")):
            with self.assertRaises(RuntimeError):
                worker.start(self.root, "0.15.0", self.data)
        self.assertEqual(worker.snapshot(self.root)["phase"], "failed")

    def test_process_lock_refuses_concurrent_start(self):
        with worker.locked(self.root / "start.lock"), \
             patch.object(worker.shutil, "which", return_value="/usr/bin/tmux"):
            with self.assertRaisesRegex(RuntimeError, "already running"):
                worker.start(self.root, "0.15.0", self.data)

    def test_container_exposes_only_private_source_cache_and_readonly_tools(self):
        args = worker.container_arguments("/usr/bin/docker", self.base / "source",
                                          self.base / "cache", self.base / "tools")
        self.assertIn("--cpus=8", args)
        self.assertIn("--memory=8g", args)
        self.assertIn("--memory-swap=8g", args)
        self.assertIn("--cap-drop=ALL", args)
        self.assertIn("--security-opt=no-new-privileges", args)
        mounts = [args[index + 1] for index, value in enumerate(args) if value == "--volume"]
        self.assertEqual(len(mounts), 3)
        self.assertTrue(mounts[2].endswith(":/opt/spotifast-updater:ro"))
        self.assertFalse(any("docker.sock" in value or "/.pi" in value or "/.ssh" in value for value in mounts))
        self.assertNotIn("--privileged", args)

    def test_rootless_podman_keeps_host_identity(self):
        args = worker.container_arguments("/usr/bin/podman", self.base / "source",
                                          self.base / "cache", self.base / "tools")
        self.assertIn("--userns=keep-id", args)
        self.assertIn(f"{os.getuid()}:{os.getgid()}", args)

    def test_build_timeout_does_not_start_an_agent_or_install(self):
        identity = "f" * 24
        worker.private_directory(self.root / "runs" / identity)
        self.status(run_id=identity, version="0.15.0")
        with patch.object(worker.shutil, "which", return_value="/fake/tool"), \
             patch.object(worker.shutil, "disk_usage", return_value=SimpleNamespace(free=2**40)), \
             patch.object(worker, "runtime", return_value="/fake/docker"), \
             patch.object(worker, "require_image"), patch.object(worker, "prepare_source"), \
             patch.object(worker, "integrate"), \
             patch.object(worker, "build_candidate", side_effect=TimeoutError("deadline")), \
             patch.object(worker, "pi_repair") as agent, patch.object(worker.signal, "alarm") as alarm:
            with self.assertRaises(InterruptedError):
                worker.worker(self.root, identity)
        agent.assert_not_called()
        self.assertEqual(worker.snapshot(self.root)["phase"], "failed")
        self.assertFalse(self.data.exists())
        self.assertEqual(alarm.call_args.args, (0,))

    def test_invalid_terminal_session_cannot_be_attached(self):
        self.status(phase="needs_input", session="other; touch something")
        with patch.object(worker, "command") as called:
            with self.assertRaises(RuntimeError):
                worker.attach(self.root)
        called.assert_not_called()

    def test_repairs_do_not_follow_source_symlinks(self):
        source = self.base / "source"
        source.mkdir()
        (source / "escape").symlink_to(self.base / "outside")
        with self.assertRaisesRegex(RuntimeError, "symlinks"):
            worker.tree_files(source)

    def test_ordinary_source_repair_is_applied_without_approval(self):
        source, edited = self.base / "source", self.base / "edited"
        for directory in [source, edited]:
            (directory / "src/ui").mkdir(parents=True)
        (source / "src/ui/view.rs").write_text("fn old() {}\n")
        (edited / "src/ui/view.rs").write_text("fn new() {}\n")
        before = worker.tree_files(source)
        with patch.object(worker, "timed_input") as asked:
            worker.apply_repairs(self.root, source, edited, before)
        asked.assert_not_called()
        self.assertEqual((source / "src/ui/view.rs").read_text(), "fn new() {}\n")

    def test_embedded_test_changes_need_real_terminal_approval(self):
        source, edited = self.base / "source", self.base / "edited"
        for directory in [source, edited]:
            (directory / "src/ui").mkdir(parents=True)
        original = "fn work() {}\n#[cfg(test)]\nmod tests { assert!(true); }\n"
        (source / "src/ui/view.rs").write_text(original)
        (edited / "src/ui/view.rs").write_text("fn work() {}\n#[cfg(test)]\nmod tests {}\n")
        with patch.object(worker, "command"), patch.object(worker, "timed_input", return_value="no"):
            with self.assertRaisesRegex(RuntimeError, "not approved"):
                worker.apply_repairs(self.root, source, edited, worker.tree_files(source))
        self.assertEqual((source / "src/ui/view.rs").read_text(), original)
        self.assertEqual(worker.snapshot(self.root)["phase"], "needs_input")

    def test_authentication_changes_need_review(self):
        source, edited = self.base / "source", self.base / "edited"
        for directory in [source, edited]:
            (directory / "src").mkdir(parents=True)
        (source / "src/auth.rs").write_text("original")
        (edited / "src/auth.rs").write_text("changed")
        with patch.object(worker, "command"), patch.object(worker, "timed_input", return_value="no"):
            with self.assertRaisesRegex(RuntimeError, "not approved"):
                worker.apply_repairs(self.root, source, edited, worker.tree_files(source))
        self.assertEqual((source / "src/auth.rs").read_text(), "original")

    def test_control_command_has_a_real_deadline(self):
        started = time.monotonic()
        with self.assertRaises(TimeoutError):
            worker.command(["sleep", "5"], timeout=0.05, capture=True)
        self.assertLess(time.monotonic() - started, 2)

    def test_silent_build_is_stopped_without_waiting_for_a_line(self):
        started = time.monotonic()
        with self.assertRaises(TimeoutError):
            worker.logged_command(["sleep", "5"], self.root / "test.log",
                                  timeout=1, idle_timeout=0.05)
        self.assertLess(time.monotonic() - started, 2)

    def test_build_total_deadline_is_independent_of_idle_limit(self):
        with self.assertRaises(TimeoutError):
            worker.logged_command(["sleep", "5"], self.root / "test.log",
                                  timeout=0.05, idle_timeout=10)

    def test_input_timeout_stops_instead_of_retrying_forever(self):
        with patch.object(worker.select, "select", return_value=([], [], [])), \
             patch.object(worker.sys, "stdout", io.StringIO()):
            with self.assertRaises(InterruptedError):
                worker.timed_input("Question: ")

    def test_retries_stop_after_three_attempts(self):
        with patch.object(worker, "need_input") as asked, \
             patch.object(worker, "runtime", side_effect=RuntimeError("unavailable")) as operation:
            with self.assertRaisesRegex(RuntimeError, "failed three times"):
                worker.retry(self.root, "Runtime", operation)
        self.assertEqual(operation.call_count, 3)
        self.assertEqual(asked.call_count, 2)

    def candidate(self):
        identity = "b" * 24
        run = self.root / "runs" / identity
        (run / "output").mkdir(parents=True)
        image = run / "output/update.AppImage"
        header = bytearray(2048)
        header[:4] = b"\x7fELF"
        header[8:11] = b"AI\x02"
        image.write_bytes(header)
        worker.atomic_json(run / "candidate.json", {"schema": 1, "version": "0.15.0",
                           "sha256": worker.digest(image), "revision": "c" * 40})
        self.status(phase="ready", version="0.15.0", run_id=identity, data_dir=str(self.data))
        return run, image

    def test_activation_requires_checked_state(self):
        self.status(phase="building")
        with patch.object(worker.subprocess, "Popen") as spawned:
            with self.assertRaisesRegex(RuntimeError, "No checked"):
                worker.activate(self.root, os.getpid())
        spawned.assert_not_called()
        self.assertFalse(self.data.exists())

    def test_changed_candidate_cannot_be_activated(self):
        _, image = self.candidate()
        with image.open("ab") as changed:
            changed.write(b"changed")
        with self.assertRaisesRegex(RuntimeError, "changed after"):
            worker.activate(self.root, os.getpid())
        self.assertFalse(self.data.exists())

    def test_unmanaged_selector_is_never_overwritten(self):
        self.candidate()
        self.data.mkdir()
        (self.data / "current").write_text("keep this")
        with self.assertRaisesRegex(RuntimeError, "unmanaged"):
            worker.activate(self.root, os.getpid())
        self.assertEqual((self.data / "current").read_text(), "keep this")

    def test_activation_retains_previous_and_does_not_kill_running_app(self):
        self.candidate()
        old = self.base / "old-generation"
        old.mkdir()
        self.data.mkdir()
        (self.data / "current").symlink_to(old)
        with patch.object(worker.subprocess, "Popen") as spawned, \
             patch.object(worker, "git") as git:
            worker.activate(self.root, os.getpid())
        git.assert_not_called()
        self.assertEqual((self.data / "previous").resolve(), old)
        self.assertTrue((self.data / "current/run").is_file())
        self.assertTrue((self.data / "current/spotifast.AppImage").is_file())
        args = spawned.call_args.args[0]
        self.assertIn("restart", args)
        self.assertNotIn("kill", args)
        self.assertEqual(worker.snapshot(self.root)["phase"], "installing")

    def test_supervisor_failure_restores_both_selectors(self):
        self.candidate()
        old, older = self.base / "old", self.base / "older"
        old.mkdir()
        older.mkdir()
        self.data.mkdir()
        (self.data / "current").symlink_to(old)
        (self.data / "previous").symlink_to(older)
        with patch.object(worker.subprocess, "Popen", side_effect=OSError("process limit")):
            with self.assertRaises(OSError):
                worker.activate(self.root, os.getpid())
        self.assertEqual((self.data / "current").resolve(), old)
        self.assertEqual((self.data / "previous").resolve(), older)
        self.assertEqual(worker.snapshot(self.root)["phase"], "ready")

    def test_pi_uses_only_explicit_sandbox_tools_and_a_deadline(self):
        run = worker.private_directory(self.root / "runs" / ("d" * 24))
        (run / "tools").mkdir()
        source = run / "source"
        (source / "src/ui").mkdir(parents=True)
        (source / "src/ui/view.rs").write_text("fn navigation() {}")
        self.status(version="0.15.0")
        calls = []
        def fake(arguments, **options):
            calls.append((arguments, options))
            if arguments[0] == "/fake/pi":
                worker.atomic_json(run / "pi-outcome.json", {"result": "complete"})
            return SimpleNamespace(returncode=0, stdout="", stderr="")
        with patch.object(worker.shutil, "which", return_value="/fake/pi"), \
             patch.object(worker, "command", side_effect=fake):
            worker.pi_repair(self.root, run, source, "/fake/docker", self.root / "cache", "conflict")
        invocation, options = next(call for call in calls if call[0][0] == "/fake/pi")
        self.assertEqual(options["timeout"], worker.PI_TIMEOUT)
        self.assertTrue(options["interactive"])
        self.assertIn("--no-context-files", invocation)
        self.assertIn("--no-mcp", invocation)
        self.assertIn("--no-extensions", invocation)
        self.assertIn("bash,fork_update_outcome", invocation)
        container = calls[0][0]
        self.assertIn("--network=none", container)
        self.assertIn("--read-only", container)
        self.assertNotIn("--privileged", container)
        self.assertIn("--force", calls[-1][0])
        self.assertEqual(calls[-1][1]["timeout"], 15)

    def test_state_symlinks_and_oversized_metadata_are_rejected(self):
        path = self.root / "state.json"
        other = self.base / "other.json"
        other.write_text('{"schema":1,"phase":"ready"}')
        path.symlink_to(other)
        with self.assertRaisesRegex(RuntimeError, "symlink"):
            worker.snapshot(self.root)
        path.unlink()
        path.write_bytes(b" " * 65537)
        with self.assertRaisesRegex(RuntimeError, "size limit"):
            worker.snapshot(self.root)


class IntegrationTests(unittest.TestCase):
    def test_conflict_resolution_is_private_and_retry_is_idempotent(self):
        with tempfile.TemporaryDirectory(prefix="spotifast-git-fixture-") as temporary:
            base = Path(temporary)
            upstream, fork = base / "upstream", base / "fork"
            root = worker.private_directory(base / "state")
            run = worker.private_directory(root / "runs" / ("e" * 24))
            original_command = worker.command
            def quiet(arguments, **options):
                options["capture"] = True
                options["timeout"] = 10
                return original_command(arguments, **options)
            with patch.object(worker, "command", side_effect=quiet), \
                 patch.object(worker, "FORK", str(fork)), patch.object(worker, "UPSTREAM", str(upstream)):
                worker.git(["init", "-b", "main", str(upstream)], cwd=base)
                (upstream / "src/ui").mkdir(parents=True)
                file = "src/ui/view.rs"
                (upstream / file).write_text("fn navigation() { old(); }\n")
                worker.git(["add", "."], cwd=upstream)
                worker.git(["commit", "-m", "base"], cwd=upstream)
                worker.git(["clone", str(upstream), str(fork)], cwd=base)
                (fork / file).write_text("fn navigation() { vim(); }\n")
                worker.git(["commit", "-am", "fork navigation"], cwd=fork)
                (upstream / file).write_text("fn navigation() { upstream(); }\n")
                worker.git(["commit", "-am", "upstream API"], cwd=upstream)
                worker.git(["tag", "v0.15.0"], cwd=upstream)
                worker.publish(root, version="0.15.0")
                source = worker.prepare_source(root, run)
                self.assertEqual(worker.prepare_source(root, run), source)
                def resolve(_root, _run, checkout, *_args):
                    (checkout / file).write_text("fn navigation() { vim(); upstream(); }\n")
                with patch.object(worker, "pi_repair", side_effect=resolve) as repaired:
                    worker.integrate(root, run, source, "fake", root / "cache")
                    self.assertEqual(repaired.call_count, 1)
                    revision = worker.git(["rev-parse", "HEAD"], cwd=source, capture=True).stdout
                    worker.integrate(root, run, source, "fake", root / "cache")
                    self.assertEqual(repaired.call_count, 1)
                    self.assertEqual(worker.git(["rev-parse", "HEAD"], cwd=source, capture=True).stdout, revision)
                self.assertIn("vim(); upstream();", (source / file).read_text())
                self.assertEqual((fork / file).read_text(), "fn navigation() { vim(); }\n")
                self.assertEqual((upstream / file).read_text(), "fn navigation() { upstream(); }\n")
                self.assertFalse((run / "merge.json").exists())


class IdentityTests(unittest.TestCase):
    def test_identity_validation_is_transactional(self):
        with tempfile.TemporaryDirectory(prefix="spotifast-identity-test-") as temporary:
            root = Path(temporary)
            (root / "first").write_text("old")
            (root / "second").write_text("unexpected")
            recipe = root / "recipe.json"
            recipe.write_text(json.dumps([
                {"path": "first", "replacements": [["old", "new"]]},
                {"path": "second", "replacements": [["old", "new"]]},
            ]))
            with self.assertRaisesRegex(RuntimeError, "changed upstream"):
                builder.apply_identity(root, recipe)
            self.assertEqual((root / "first").read_text(), "old")

    def test_identity_recipe_cannot_escape_source(self):
        with tempfile.TemporaryDirectory(prefix="spotifast-identity-test-") as temporary:
            root = Path(temporary)
            recipe = root / "recipe.json"
            recipe.write_text('[{"path":"../outside","replacements":[]}]')
            with self.assertRaisesRegex(RuntimeError, "escapes"):
                builder.apply_identity(root, recipe)


if __name__ == "__main__":
    unittest.main()
