"""YM_Link bootstrap checks and local environment diagnostics (Python 3.10+)."""

from __future__ import annotations

import argparse
from graphlib import CycleError, TopologicalSorter
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
REQUIRED = (
    "README.md", "AGENTS.md", "docs/product/YM-LINK.md", "docs/engineering/ARCHITECTURE.md",
    "docs/engineering/PROTOCOL.md", "docs/harness/README.md", "docs/harness/IMPROVEMENT.md",
    "docs/README.md", "docs/engineering/TESTING.md", "docs/product/ROADMAP.md",
    "docs/STATUS.md", "docs/engineering/decisions/0001-platform.md",
    "docs/harness/templates/task.md", ".github/workflows/repository-checks.yml",
    "Cargo.toml", "core/Cargo.toml",
)
STATES = {"ready", "doing", "verifying", "blocked", "done", "superseded"}
WORKFLOWS = ("maintenance", "behavior", "feature")
# Every YML task is under the numbering contract; duplicates are rejected.
TASK_CONTRACT_START = 1


def run(args: list[str], timeout: int = 30, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args, cwd=cwd or ROOT, capture_output=True, text=True, encoding="utf-8",
        errors="replace", timeout=timeout, check=False,
    )


def markdown_structure(content: str) -> str:
    """Mask fenced examples while preserving offsets for extracting real section bodies."""
    lines = []
    fence = ""
    for line in content.splitlines(keepends=True):
        inside = bool(fence)
        marker = re.match(r" {0,3}(`{3,}|~{3,})([^\n]*)", line)
        if marker:
            if not fence:
                fence = marker[1]
            elif marker[1][0] == fence[0] and len(marker[1]) >= len(fence) and not marker[2].strip():
                fence = ""
        lines.append(re.sub(r"[^\n]", " ", line) if inside or fence else line)
    return "".join(lines)


def links(path: Path, content: str) -> list[Path]:
    """Resolve inline Markdown file links; URL availability/anchors are not checked."""
    prose = markdown_structure(content)
    targets = []
    for target in re.findall(r"\[[^\]]*\]\(([^\n)]+)\)", prose):
        target = target.strip().strip("<>")
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc or not parsed.path:
            continue
        value = unquote(parsed.path)
        targets.append(
            ((ROOT / value.lstrip("/")) if value.startswith("/") else path.parent / value).resolve()
        )
    return targets


def field(content: str, name: str) -> str:
    match = re.search(rf"^(?:- )?{re.escape(name)}:[ \t]*(.*)$", content, re.M)
    return match.group(1).strip() if match else ""


def task_header(content: str) -> str:
    return re.split(r"^##(?:[ \t]+|$)", markdown_structure(content), maxsplit=1, flags=re.M)[0]


def task_paths() -> list[Path]:
    return sorted((ROOT / "docs/tasks").rglob("YML-*.md"))


def task_number(path: Path) -> int:
    match = re.fullmatch(r"YML-(\d+)-.+\.md", path.name)
    if not match:
        raise ValueError(f"Invalid task filename: {path.name}")
    return int(match[1])


def task_inventory_errors(paths: list[Path]) -> list[str]:
    seen: dict[int, Path] = {}
    errors = []
    successors = {}
    for path in paths:
        number = task_number(path)
        if number >= TASK_CONTRACT_START and number in seen:
            errors.append(f"duplicate YML-{number}: {seen[number].name}, {path.name}")
        seen[number] = path
        header = task_header(path.read_text(encoding="utf-8"))
        if field(header, "Status") == "superseded":
            successors[path.resolve()] = links(path, field(header, "Superseded-By"))
    try:
        TopologicalSorter(successors).prepare()
    except CycleError as error:
        errors.append("supersession cycle: " + " -> ".join(p.name for p in error.args[1]))
    return errors


def tasks(include_archived: bool) -> int:
    for path in task_paths():
        if not include_archived and "archive" in path.relative_to(ROOT / "docs/tasks").parts:
            continue
        content = path.read_text(encoding="utf-8")
        title = next((line.lstrip("# ") for line in content.splitlines() if line.startswith("# ")), path.stem)
        header = task_header(content)
        print(f"{field(header, 'Status'):10} {path.relative_to(ROOT).as_posix()} | {title}")
    return 0


def new_task(slug: str, title: str, workflow: str, requires_device: bool) -> Path:
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", slug):
        raise ValueError("Task topic must use lowercase words joined by hyphens")
    if not title.strip() or any(char in title for char in "\r\n") or workflow not in WORKFLOWS:
        raise ValueError("Task needs a single-line title and a known workflow")
    folder = ROOT / "docs/tasks"
    if not folder.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("Task directory must remain inside the repository")
    number = max([TASK_CONTRACT_START - 1, *(task_number(p) for p in task_paths())]) + 1
    content = (ROOT / "docs/harness/templates/task.md").read_text(encoding="utf-8")
    content = content.replace("YML-NNN", f"YML-{number:03d}").replace("任务名称", title.strip())
    content = content.replace("Workflow: maintenance", f"Workflow: {workflow}")
    content = content.replace("Requires-Device: no", f"Requires-Device: {'yes' if requires_device else 'no'}")
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / f"YML-{number:03d}-{slug}.md"
    # ponytail: single-writer creation; concurrent creators need an atomic ID allocator.
    with path.open("x", encoding="utf-8", newline="\n") as target:
        target.write(content)
    return path


def check_task(path: Path, content: str) -> list[str]:
    errors = []
    label = path.relative_to(ROOT).as_posix()
    archived = "archive" in path.relative_to(ROOT / "docs/tasks").parts
    structure = markdown_structure(content)
    # Examples and body prose cannot provide task metadata or section headings.
    header = task_header(content)
    state = field(header, "Status")
    device = field(header, "Requires-Device")
    if state not in STATES:
        errors.append(f"{label}: invalid or missing Status")
    if device not in {"yes", "no"}:
        errors.append(f"{label}: Requires-Device must be yes or no")
    declared = field(header, "Tests")
    if archived and state not in {"done", "superseded"}:
        errors.append(f"{label}: archived task must be done or superseded")
    if not archived and state in {"done", "superseded"}:
        errors.append(f"{label}: terminal task must be archived")
    if state == "superseded":
        successors = links(path, field(header, "Superseded-By"))
        if not successors or any(
            not p.is_relative_to((ROOT / "docs/tasks").resolve()) or not p.is_file()
            or p == path.resolve() or not re.fullmatch(r"YML-\d+-.+\.md", p.name)
            for p in successors
        ):
            errors.append(f"{label}: Superseded-By must link to existing successor tasks")
    if task_number(path) >= TASK_CONTRACT_START:
        workflow = field(header, "Workflow")
        if workflow not in WORKFLOWS:
            errors.append(f"{label}: Workflow must be maintenance, behavior or feature")
        for heading in ("Verification", *(('Spec',) if workflow in {"behavior", "feature"} else ())):
            section = re.search(rf"^## {heading}[ \t]*\n(.*?)(?=^## |\Z)", structure, re.M | re.S)
            if not section or not content[section.start(1):section.end(1)].strip():
                errors.append(f"{label}: missing or empty {heading} section")
        if declared.startswith("not-applicable") and not re.fullmatch(r"not-applicable \(\s*\S.*\)", declared):
            errors.append(f"{label}: Tests not-applicable needs a reason in parentheses")
    if not archived and not declared:
        errors.append(
            f"{label}: Tests field is required for active tasks "
            "(list cargo test targets, or 'not-applicable (reason)')"
        )
    for heading in ("Scope", "Acceptance", "Evidence", "Next"):
        if not re.search(rf"^## {heading}[ \t]*$", structure, re.M):
            errors.append(f"{label}: missing {heading} section")
    acceptance = re.search(r"^## Acceptance[ \t]*\n(.*?)(?=^## |\Z)", structure, re.M | re.S)
    if not acceptance or not re.search(r"^- \[[ xX]\] .+", acceptance.group(1), re.M):
        errors.append(f"{label}: no acceptance checklist")
    if state == "done":
        if re.search(r"^- \[ \]", structure, re.M):
            errors.append(f"{label}: done task still has unchecked acceptance items")
    if state in {"done", "superseded"}:
        evidence = re.search(r"^## Evidence[ \t]*\n(.*?)(?=^## |\Z)", structure, re.M | re.S)
        evidence_text = content[evidence.start(1):evidence.end(1)] if evidence else ""
        if not evidence_text.strip():
            errors.append(f"{label}: {state} task has no evidence")
        if state == "done" and device == "yes":
            reports = [
                p for p in links(path, evidence_text)
                if p.is_relative_to((ROOT / "docs/evidence").resolve()) and p.is_file()
                and p.name != "README.md"
            ]
            if not reports:
                errors.append(f"{label}: done device task needs a linked report in docs/evidence")
            for report in reports:
                data = report.read_text(encoding="utf-8")
                for name, pattern in (
                    ("Commit", r"[0-9a-fA-F]{40}"),
                    ("Host-OS", r".+"), ("Result", r"pass"),
                ):
                    value = field(data, name)
                    if name in {"Commit", "Result"}:
                        # Presentation may use inline code or append a parenthesized explanation.
                        pattern = (rf"(?:{pattern}|`{pattern}`)"
                                   r"(?:\s*(?:\([^()\r\n]*\)|（[^（）\r\n]*）))?")
                    if not re.fullmatch(pattern, value):
                        errors.append(f"{label}: report {report.name} has invalid {name}")
    return errors


def main_delivery_errors() -> list[str]:
    """Read-only delivery gate; query the server instead of trusting origin/main."""
    branch = run(["git", "branch", "--show-current"])
    if branch.returncode or branch.stdout.strip() != "main":
        return ["delivery requires the main branch"]
    status = run(["git", "status", "--porcelain", "--untracked-files=no"])
    if status.returncode or status.stdout.strip():
        return ["delivery requires committing tracked changes"]
    sources = run(["git", "ls-files", "--others", "--exclude-standard", "--",
                   "core", "app", "scripts", "docs", "design", ".github", "*.toml"])
    if sources.returncode or sources.stdout.strip():
        return ["delivery requires committing new source and documentation files"]
    head = run(["git", "rev-parse", "HEAD"])
    remote = run(["git", "ls-remote", "--exit-code", "origin", "refs/heads/main"])
    if head.returncode or remote.returncode or not remote.stdout.strip():
        return ["cannot verify actual remote main; resolve remote access and retry"]
    if head.stdout.strip() != remote.stdout.split()[0]:
        return ["HEAD differs from actual remote main; synchronize, push, and retry"]
    return []


def check(require_pushed: bool = False) -> int:
    if not shutil.which("git"):
        print("FAIL: Git is required.")
        return 1
    inventory = run(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"])
    if inventory.returncode:
        print(f"FAIL: cannot enumerate repository: {inventory.stderr.strip()}")
        return 1
    files = sorted(set(filter(None, inventory.stdout.split("\0"))))
    errors = [f"missing required file: {p}" for p in REQUIRED if not (ROOT / p).is_file()]
    if not any(p.startswith(("docs/tasks/YML-", "docs/tasks/archive/YML-"))
               and p.endswith(".md") for p in files):
        errors.append("no task record in docs/tasks")
    count = 0
    for name in files:
        path = ROOT / name
        if not path.is_file() or path.suffix not in {".md", ".py", ".yml", ".yaml", ".toml"}:
            continue
        try:
            content = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(f"{name}: not valid UTF-8")
            continue
        count += 1
        if not content.endswith("\n"):
            errors.append(f"{name}: missing final newline")
        for number, line in enumerate(content.splitlines(), 1):
            if line.rstrip() != line:
                errors.append(f"{name}:{number}: trailing whitespace")
        if path.suffix == ".md":
            for target in links(path, content):
                if not target.is_relative_to(ROOT.resolve()) or not target.exists():
                    errors.append(f"{name}: missing or out-of-repository local link: {target}")
        if name.startswith("docs/tasks/") and path.suffix == ".md" and path.name != "README.md":
            if not re.fullmatch(r"YML-\d+-.+\.md", path.name):
                errors.append(f"{name}: invalid task filename; use YML-NNN-topic.md")
            else:
                errors.extend(check_task(path, content))
    errors.extend(task_inventory_errors(task_paths()))
    for args in (["git", "diff", "--check"], ["git", "diff", "--cached", "--check"]):
        result = run(args)
        if result.returncode:
            errors.append(result.stdout.strip() or result.stderr.strip() or "git diff --check failed")
    if require_pushed:
        errors.extend(main_delivery_errors())
    for error in errors:
        print(f"FAIL: {error}")
    if errors:
        print(f"Repository check failed: {len(errors)} issue(s).")
        return 1
    print(f"PASS: repository baseline; {count} text files checked.")
    print("Scope: repository records, task contracts and documentation links; external URLs and Markdown anchors are not checked.")
    print("No Rust build, cargo test, or paired-device acceptance was performed by this command.")
    if require_pushed:
        print("PASS: main is committed and HEAD matches actual remote origin/main.")
    return 0


def doctor(require_device: bool) -> int:
    print(f"Python: {sys.version.split()[0]}")
    for name in ("git", "gh", "cargo", "rustc"):
        print(f"{name}: {shutil.which(name) or 'MISSING'}")
    print("Tool presence only; version compatibility and paired-device reachability are unverified.")
    if require_device:
        print("FAIL: paired-device checks are task-scoped; run the task's own verification commands.")
        return 1
    return 0


def verify() -> int:
    """Run the full local gate: harness tests, repo check, rustfmt and cargo tests."""
    failures = []
    harness_tests = run([sys.executable, str(ROOT / "scripts/test_harness.py")], timeout=300)
    print(harness_tests.stdout, end="")
    if harness_tests.returncode:
        failures.append("scripts/test_harness.py")
    repo_check = check()
    if repo_check:
        failures.append("harness check")
    for args in (["cargo", "fmt", "--all", "--check"],
                 ["cargo", "test", "--workspace", "--all-targets"]):
        result = run(args, timeout=900)
        print(result.stdout, end="")
        if result.returncode:
            print(result.stderr, end="")
            failures.append(" ".join(args))
    if failures:
        print(f"FAIL: verification failed: {', '.join(failures)}")
        return 1
    print("PASS: harness tests, repository check, rustfmt and cargo tests.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    repository = sub.add_parser("check", help="check the repository baseline; does not build Rust code")
    repository.add_argument("--require-pushed", action="store_true",
                            help="also verify committed main matches the actual remote main")
    listing = sub.add_parser("tasks", help="list open tasks with their full paths and acceptance state")
    listing.add_argument("--all", action="store_true", help="also show done and superseded archives")
    creation = sub.add_parser("new", help="create one task using the next unused ID, including archives")
    creation.add_argument("topic", help="lowercase words joined by hyphens")
    creation.add_argument("--title", required=True)
    creation.add_argument("--workflow", choices=WORKFLOWS, required=True)
    creation.add_argument("--requires-device", action="store_true",
                          help="mark the task as requiring a paired headset or second computer")
    diagnostic = sub.add_parser("doctor", help="inspect local development tools")
    diagnostic.add_argument("--require-device", action="store_true")
    gate = sub.add_parser("verify", help="harness tests + repo check + rustfmt + cargo test")
    args = parser.parse_args()
    try:
        if args.command == "check":
            return check(args.require_pushed)
        if args.command == "tasks":
            return tasks(args.all)
        if args.command == "new":
            path = new_task(args.topic, args.title, args.workflow, args.requires_device)
            print(f"Created {path.relative_to(ROOT).as_posix()}; fill the spec, acceptance and verification before implementation.")
            return 0
        if args.command == "verify":
            return verify()
        return doctor(args.require_device)
    except (OSError, subprocess.TimeoutExpired, ValueError) as error:
        print(f"FAIL: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
