from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")


ecotope = Path("crates/mechofly-app/src/screen_ecotope.rs")
text = ecotope.read_text(encoding="utf-8")
replacements = {
    "0xEC07_0FE": "0x0EC0_70FE",
    "0xC105_1A6": "0x0C10_51A6",
    "mix64(self.seed ^ next_bucket ^ 0xC07A_61A7) % 4 == 0":
        "mix64(self.seed ^ next_bucket ^ 0xC07A_61A7).is_multiple_of(4)",
    "mix64(self.seed ^ dropout_bucket ^ 0xD20F_0F5E) % 13 == 0":
        "mix64(self.seed ^ dropout_bucket ^ 0xD20F_0F5E).is_multiple_of(13)",
}
expected_counts = {
    "0xEC07_0FE": 2,
    "0xC105_1A6": 1,
    "mix64(self.seed ^ next_bucket ^ 0xC07A_61A7) % 4 == 0": 1,
    "mix64(self.seed ^ dropout_bucket ^ 0xD20F_0F5E) % 13 == 0": 1,
}
for old, new in replacements.items():
    count = text.count(old)
    if count != expected_counts[old]:
        raise SystemExit(f"{old}: expected {expected_counts[old]} matches, found {count}")
    text = text.replace(old, new)
ecotope.write_text(text, encoding="utf-8", newline="\n")

self_test = Path("crates/mechofly-app/src/self_test.rs")
replace_once(
    self_test,
    "    desktop_pet_topmost_with_neural_windows: bool,\n",
    "    desktop_pet_topmost_without_neural_windows: bool,\n"
    "    desktop_pet_topmost_with_neural_windows: bool,\n",
    "self-test topmost field",
)
replace_once(
    self_test,
    "        desktop_pet_topmost_with_neural_windows:\n"
    "            desktop_safety.topmost_when_observatory_open,\n",
    "        desktop_pet_topmost_without_neural_windows:\n"
    "            desktop_safety.topmost_when_observatory_closed,\n"
    "        desktop_pet_topmost_with_neural_windows:\n"
    "            desktop_safety.topmost_when_observatory_open,\n",
    "self-test topmost assignment",
)
replace_once(
    self_test,
    "    let desktop_safety = NonWindowsDesktopSafetyContract {\n"
    "        passed: true,\n"
    "        topmost_when_observatory_open: true,\n",
    "    let desktop_safety = NonWindowsDesktopSafetyContract {\n"
    "        passed: true,\n"
    "        topmost_when_observatory_closed: true,\n"
    "        topmost_when_observatory_open: true,\n",
    "non-Windows topmost initialization",
)
replace_once(
    self_test,
    "struct NonWindowsDesktopSafetyContract {\n"
    "    passed: bool,\n"
    "    topmost_when_observatory_open: bool,\n",
    "struct NonWindowsDesktopSafetyContract {\n"
    "    passed: bool,\n"
    "    topmost_when_observatory_closed: bool,\n"
    "    topmost_when_observatory_open: bool,\n",
    "non-Windows topmost contract",
)
