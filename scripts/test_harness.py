"""Regression tests for scripts/harness.py; run with `python3 scripts/test_harness.py`."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("ymlink_harness", SCRIPTS / "harness.py")
harness = importlib.util.module_from_spec(spec)
sys.modules["ymlink_harness"] = harness
spec.loader.exec_module(harness)

REAL_ROOT = harness.ROOT


class TempRepoTestCase(unittest.TestCase):
    def setUp(self):
        self._root = tempfile.TemporaryDirectory()
        self.addCleanup(self._root.cleanup)
        self.root = Path(self._root.name)
        self._old_root = harness.ROOT
        harness.ROOT = self.root
        self.addCleanup(setattr, harness, "ROOT", self._old_root)
        (self.root / "docs/harness/templates").mkdir(parents=True)
        (self.root / "docs/harness/templates/task.md").write_text(
            "# YML-NNN：任务名称\n\nStatus: ready\nWorkflow: maintenance\nRequires-Device: no\n"
            "Tests: not-applicable (示例原因)\n\n## Scope\n\n示例\n\n## Spec\n\n示例\n\n"
            "## Acceptance\n\n- [ ] 示例\n\n## Verification\n\n示例\n\n## Evidence\n\n示例\n\n"
            "## Next\n\n示例\n", encoding="utf-8")

    def write_task(self, name: str, **fields) -> Path:
        path = self.root / "docs/tasks" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        body = fields.pop("body", "正文")
        header = {
            "Status": "ready", "Workflow": "maintenance", "Requires-Device": "no",
            "Tests": "not-applicable (测试原因)", **fields,
        }
        lines = [f"# {name}：测试"]
        lines += [f"{key}: {value}" for key, value in header.items()]
        sections = {
            "Scope": "范围", "Acceptance": "- [x] 已完成项", "Verification": "检查",
            "Evidence": "证据", "Next": "下一步",
        }
        for key, value in fields.items():
            if key.startswith("section-"):
                sections[key.removeprefix("section-")] = value
        text = "\n".join(lines) + "\n\n"
        for heading, content in sections.items():
            if content is not None:
                text += f"## {heading}\n\n{content}\n\n"
        text += body + "\n"
        path.write_text(text, encoding="utf-8")
        return path


class MarkdownHelpers(unittest.TestCase):
    def test_fenced_examples_are_masked_but_offsets_preserved(self):
        content = "正文\n\n```\nStatus: doing\n```\n\nStatus: ready\n"
        masked = harness.markdown_structure(content)
        self.assertNotIn("doing", masked.replace("\n", "").replace(" ", ""))
        self.assertEqual(masked, "正文\n\n" + " " * len("```") + "\n" + " " * len("Status: doing") + "\n"
                         + " " * 3 + "\n\nStatus: ready\n")

    def test_links_skip_urls_and_resolve_relative(self):
        path = harness.ROOT / "docs/README.md"
        targets = harness.links(path, "看 [状态](STATUS.md) 与 [仓库](https://github.com/forrestzhu/youmu-link)。")
        self.assertEqual(targets, [(path.parent / "STATUS.md").resolve()])


class TaskInventory(TempRepoTestCase):
    def test_invalid_filename_rejected(self):
        with self.assertRaises(ValueError):
            harness.task_number(self.root / "docs/tasks/ym-1-bad.md")

    def test_duplicate_numbers_reported(self):
        self.write_task("YML-001-first.md")
        self.write_task("YML-001-second.md")
        errors = harness.task_inventory_errors(harness.task_paths())
        self.assertTrue(any("duplicate YML-1" in error for error in errors))

    def test_supersession_cycle_reported(self):
        first = self.write_task("YML-001-first.md", Status="superseded",
                                **{"Superseded-By": "[继任](YML-002-second.md)"})
        self.write_task("YML-002-second.md", Status="superseded",
                        **{"Superseded-By": "[继任](YML-001-first.md)"})
        errors = harness.task_inventory_errors(harness.task_paths())
        self.assertTrue(any("supersession cycle" in error for error in errors), str(errors))
        self.assertTrue(first.exists())


class NewTask(TempRepoTestCase):
    def test_allocates_next_id_including_archive(self):
        self.write_task("YML-001-old.md")
        (self.root / "docs/tasks/archive").mkdir(parents=True)
        self.write_task("archive/YML-007-archived.md")
        path = harness.new_task("demo-work", "演示任务", "feature", True)
        self.assertEqual(path.name, "YML-008-demo-work.md")
        content = path.read_text(encoding="utf-8")
        self.assertIn("# YML-008：演示任务", content)
        self.assertIn("Workflow: feature", content)
        self.assertIn("Requires-Device: yes", content)

    def test_slug_and_title_validation(self):
        with self.assertRaises(ValueError):
            harness.new_task("Bad Slug", "标题", "maintenance", False)
        with self.assertRaises(ValueError):
            harness.new_task("ok-slug", "多行\n标题", "maintenance", False)
        with self.assertRaises(ValueError):
            harness.new_task("ok-slug", "标题", "unknown", False)


class CheckTask(TempRepoTestCase):
    def errors_for(self, name: str, **fields) -> list[str]:
        path = self.write_task(name, **fields)
        return harness.check_task(path, path.read_text(encoding="utf-8"))

    def test_valid_ready_task_passes(self):
        self.assertEqual(self.errors_for("YML-001-ok.md"), [])

    def test_metadata_fields_enforced(self):
        self.assertTrue(any("Status" in error for error in self.errors_for("YML-001-bad.md", Status="oops")))
        self.assertTrue(any("Requires-Device" in error
                            for error in self.errors_for("YML-001-bad.md", **{"Requires-Device": ""})))
        self.assertTrue(any("Workflow" in error for error in self.errors_for("YML-001-bad.md", Workflow="sprint")))

    def test_active_task_requires_tests_declaration(self):
        errors = self.errors_for("YML-001-bad.md", Tests="")
        self.assertTrue(any("Tests field is required" in error for error in errors))
        errors = self.errors_for("YML-001-bad.md", Tests="not-applicable 没有括号理由")
        self.assertTrue(any("not-applicable needs a reason" in error for error in errors))

    def test_feature_requires_spec_section(self):
        errors = self.errors_for("YML-001-bad.md", Workflow="feature", **{"section-Spec": None})
        self.assertTrue(any("missing or empty Spec section" in error for error in errors))

    def test_required_sections_and_checklist(self):
        errors = self.errors_for("YML-001-bad.md", **{"section-Acceptance": "没有清单"})
        self.assertTrue(any("no acceptance checklist" in error for error in errors))
        errors = self.errors_for("YML-001-bad.md", **{"section-Next": None})
        self.assertTrue(any("missing Next section" in error for error in errors))

    def test_terminal_states_need_archive_and_evidence(self):
        errors = self.errors_for("YML-001-bad.md", Status="done")
        self.assertTrue(any("terminal task must be archived" in error for error in errors))
        archived = self.write_task("archive/YML-002-done.md", Status="done",
                                   **{"section-Acceptance": "- [ ] 未完成项", "section-Evidence": "证据"})
        content = archived.read_text(encoding="utf-8")
        errors = harness.check_task(archived, content)
        self.assertTrue(any("still has unchecked acceptance items" in error for error in errors))
        empty = self.write_task("archive/YML-003-done.md", Status="done", **{"section-Evidence": ""})
        errors = harness.check_task(empty, empty.read_text(encoding="utf-8"))
        self.assertTrue(any("has no evidence" in error for error in errors))

    def test_done_device_task_needs_bound_report(self):
        task = self.write_task("archive/YML-004-dev.md", Status="done",
                               **{"Requires-Device": "yes",
                                  "section-Evidence": "- 报告 [r](../../evidence/2026-01-01-report.md)"})
        errors = harness.check_task(task, task.read_text(encoding="utf-8"))
        self.assertTrue(any("needs a linked report" in error for error in errors))
        report_dir = self.root / "docs/evidence"
        report_dir.mkdir(parents=True)
        report = report_dir / "2026-01-01-report.md"
        report.write_text("- Commit: 0123456789abcdef0123456789abcdef01234567\n"
                          "- Host-OS: macOS 26\n- Result: pass\n", encoding="utf-8")
        errors = harness.check_task(task, task.read_text(encoding="utf-8"))
        self.assertEqual(errors, [])
        report.write_text("- Commit: short\n- Host-OS: macOS 26\n- Result: pass\n", encoding="utf-8")
        errors = harness.check_task(task, task.read_text(encoding="utf-8"))
        self.assertTrue(any("invalid Commit" in error for error in errors))

    def test_superseded_by_must_link_existing_task(self):
        errors = self.errors_for("archive/YML-005-sup.md", Status="superseded",
                                 **{"Superseded-By": "[继任](missing.md)"})
        self.assertTrue(any("Superseded-By must link" in error for error in errors))


class RealRepository(unittest.TestCase):
    def test_repository_check_passes(self):
        self.assertEqual(harness.check(), 0)


if __name__ == "__main__":
    unittest.main(verbosity=2)
