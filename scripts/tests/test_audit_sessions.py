import json
import subprocess
import sys
import unittest
from pathlib import Path


class TestAuditSessions(unittest.TestCase):
    def test_claude_fixtures_json(self) -> None:
        tests_dir = Path(__file__).resolve().parent
        script_path = tests_dir.parent / "audit-sessions.py"
        claude_fixtures = tests_dir / "fixtures" / "claude"
        missing_ag = tests_dir / "fixtures" / "nonexistent-antigravity"

        proc = subprocess.run(
            [
                sys.executable,
                "-I",
                str(script_path),
                "--claude",
                str(claude_fixtures),
                "--antigravity",
                str(missing_ag),
                "--json",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        data = json.loads(proc.stdout)
        self.assertIn("proj", data)

        keys = ("ci_mcp", "ci_cli", "read", "search_shell")
        filtered = {
            agent: {k: counts.get(k, 0) for k in keys if counts.get(k, 0) != 0}
            for agent, counts in data["proj"].items()
        }
        expected = {
            "main": {"ci_mcp": 2, "read": 1},
            "subagent:Explore": {"ci_cli": 1, "search_shell": 1},
        }
        self.assertEqual(filtered, expected)
        self.assertEqual(data["proj"]["main"].get("files"), 1)
        self.assertEqual(data["proj"]["subagent:Explore"].get("files"), 1)
        self.assertEqual(data["proj"]["subagent:Explore"].get("bash"), 2)
        self.assertEqual(data.get("antigravity"), {"dbs": 0, "tools": {}})


if __name__ == "__main__":
    unittest.main()
