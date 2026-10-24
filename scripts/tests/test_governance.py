"""Repository governance files and the secret flow into release.yml (text checks; no YAML library needed)."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RELEASE = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
CUT = (ROOT / ".github/workflows/cut-release.yml").read_text(encoding="utf-8")
SECRETS = ["PYXEE_DISCORD_CLIENT_ID", "PYXEE_LASTFM_API_KEY", "PYXEE_LASTFM_API_SECRET", "WINDOWS_CERT_PFX_BASE64", "WINDOWS_CERT_PASSWORD", "ANTHROPIC_API_KEY", "AI_API_KEY"]


def block(text: str, header: str) -> str:
    """The indented block that follows `header` (a line ending in a colon)."""
    lines = text.splitlines()
    start = next(i for i, l in enumerate(lines) if l.strip() == header)
    indent = len(lines[start]) - len(lines[start].lstrip())
    out = []
    for l in lines[start + 1:]:
        if l.strip() and len(l) - len(l.lstrip()) <= indent:
            break
        out.append(l)
    return "\n".join(out)


class Security(unittest.TestCase):
    def test_security_md_uses_private_reporting_and_invents_no_address(self):
        text = (ROOT / "SECURITY.md").read_text(encoding="utf-8")
        self.assertIn("private vulnerability reporting", text.lower())
        self.assertIn("Report a vulnerability", text)
        self.assertNotRegex(text, r"[\w.+-]+@[\w-]+\.[\w.]+", "no email address: none was provided")

    def test_codeowners_names_the_owner(self):
        text = (ROOT / ".github/CODEOWNERS").read_text(encoding="utf-8")
        self.assertRegex(text, r"(?m)^\*\s+@Pylxyr\s*$")


class SecretFlow(unittest.TestCase):
    def test_cut_release_no_longer_inherits_everything(self):
        self.assertNotRegex(CUT, r"(?m)^\s*secrets:\s*inherit\b")

    def test_release_declares_exactly_the_secrets_it_uses(self):
        declared = block(block(RELEASE, "workflow_call:"), "secrets:")
        names = re.findall(r"(?m)^\s{6}([A-Z0-9_]+):\s*$", declared)
        self.assertEqual(sorted(names), sorted(SECRETS))
        self.assertEqual(declared.count("required: false"), len(SECRETS), "optional: unset behaves as before")
        used = set(re.findall(r"secrets\.([A-Z0-9_]+)", RELEASE))
        self.assertEqual(used, set(SECRETS), "release.yml uses a secret that is not declared, or declares one it never uses")

    def test_cut_release_passes_each_declared_secret_explicitly(self):
        passed = block(CUT, "secrets:")
        for name in SECRETS:
            self.assertRegex(passed, rf"(?m)^\s+{name}: \${{{{ secrets\.{name} }}}}\s*$")
        self.assertEqual(len(re.findall(r"(?m)^\s+[A-Z0-9_]+: \$\{\{ secrets\.", passed)), len(SECRETS))

    def test_tag_push_and_manual_runs_still_use_repository_secrets_directly(self):
        # Those triggers need no declaration: the secrets context comes from the repository.
        self.assertIn("push:", RELEASE)
        self.assertIn("workflow_dispatch:", RELEASE)
        for name in SECRETS:
            self.assertIn(f"secrets.{name}", RELEASE)


if __name__ == "__main__":
    unittest.main()
