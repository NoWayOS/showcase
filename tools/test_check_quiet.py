# SPDX-License-Identifier: MIT OR Apache-2.0
"""Tests for tools/check-quiet: a fixture that passes, then each rule
broken on its own, so each is shown to fail.

Run: python3 -B -m unittest discover -s tools -p 'test_*.py'   (make quiet-check)
Python 3.11 or later, standard library only.
"""

import importlib.machinery
import importlib.util
import os
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))


def load(name):
    """Import an extensionless script in this directory as a module."""
    loader = importlib.machinery.SourceFileLoader(name.replace("-", "_"), os.path.join(HERE, name))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


check_quiet = load("check-quiet")

SYSTEM = """<?xml version="1.0" encoding="UTF-8"?>
<system>
    <protection_domain name="ui">
        <program_image path="{image}" />
    </protection_domain>
</system>
"""
EXPECT = "SHOWCASE: ui: showing welcome\n!display-stub: frame\n!zqvj\n!9081726354\n"
SCRIPT = "wait SHOWCASE: ui: showing step-text\nserial zqvj\\t9081726354\\e[Z\\x20\\r\n"
DISPLAY = "fn init() -> Handler {\n    nowayos_display_stub::init_quiet()\n}\n"


class CheckQuiet(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        root = self.dir.name
        self.systems = os.path.join(root, "systems")
        self.pds = os.path.join(root, "pds")
        self.write("systems/demo.system", SYSTEM.format(image="showcase-ui.elf"))
        self.write("systems/demo.expect", EXPECT)
        self.write("systems/demo.qmp", SCRIPT)
        self.write("systems/rpi5b_2gb/demo.system", SYSTEM.format(image="showcase-ui.elf"))
        self.write("systems/rpi5b_2gb/demo.expect", EXPECT)
        self.write("pds/display/src/main.rs", DISPLAY)

    def write(self, path, text):
        path = os.path.join(self.dir.name, path)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(text)

    def remove(self, path):
        os.remove(os.path.join(self.dir.name, path))

    def problems(self):
        return check_quiet.problems(self.systems, self.pds)

    def assert_fails(self, needle):
        found = self.problems()
        self.assertTrue(any(needle in problem for problem in found), found)

    def test_the_fixture_passes(self):
        self.assertEqual(self.problems(), [])

    def test_a_program_not_showcases_fails(self):
        self.write("systems/rpi5b_2gb/demo.system", SYSTEM.format(image="nowayos-display-stub.elf"))
        self.assert_fails("runs nowayos-display-stub.elf")

    def test_a_product_s_programs_pass_under_its_own_prefix_only(self):
        # A product built on the Showcase checks its systems with its
        # own programs' prefix; the Showcase's own then fail there.
        for path in ("systems/demo.system", "systems/rpi5b_2gb/demo.system"):
            self.write(path, SYSTEM.format(image="product-ui.elf"))
        self.assertEqual(check_quiet.problems(self.systems, self.pds, "product"), [])
        self.assert_fails("runs product-ui.elf")
        self.write("systems/demo.system", SYSTEM.format(image="showcase-ui.elf"))
        found = check_quiet.problems(self.systems, self.pds, "product")
        self.assertTrue(any("runs showcase-ui.elf" in problem for problem in found), found)

    def test_a_display_pd_not_quiet_fails(self):
        self.write("pds/display/src/main.rs", DISPLAY.replace("init_quiet()", "init()"))
        self.assert_fails("the display PD must run quiet")

    def test_a_ramfb_display_pd_not_quiet_fails(self):
        self.write("pds/display-ramfb/src/main.rs",
                   "fn init() -> DisplayPd {\n    nowayos_display_ramfb::init()\n}\n")
        self.assert_fails("pds/display-ramfb/src/main.rs: the display PD must run quiet")

    def test_a_ramfb_system_must_forbid_the_ramfb_hash(self):
        self.write("systems/rpi5b_2gb/demo.system", SYSTEM.format(image="showcase-display-ramfb.elf"))
        self.assert_fails("must forbid 'NOWAYOS: display: showing 0x'")

    def test_an_expect_file_allowing_hashes_fails(self):
        self.write("systems/rpi5b_2gb/demo.expect", EXPECT.replace("!display-stub: frame\n", ""))
        self.assert_fails("must forbid 'display-stub: frame'")

    def test_a_system_with_no_expect_file_fails(self):
        self.remove("systems/demo.expect")
        self.assert_fails("no demo.expect")

    def test_no_systems_fails(self):
        for path in ("systems/demo.system", "systems/rpi5b_2gb/demo.system"):
            self.remove(path)
        self.assert_fails("no systems")

    def test_an_unmarked_reference_fails(self):
        self.write("systems/demo.qmp", SCRIPT.replace("9081726354", "42"))
        self.assert_fails("types '42'")

    def test_a_marker_one_expect_file_allows_fails(self):
        self.write("systems/rpi5b_2gb/demo.expect", EXPECT.replace("!9081726354\n", ""))
        self.assert_fails("types '9081726354'")

    def test_text_after_a_space_is_a_run_of_its_own(self):
        self.write("systems/demo.qmp", SCRIPT.replace("zqvj", "zqvj\\x20smith"))
        self.assert_fails("types 'smith'")

    def test_a_marker_inside_a_run_is_not_enough(self):
        self.write("systems/demo.qmp", SCRIPT.replace("zqvj", "bozqvj"))
        self.assert_fails("types 'bozqvj'")

    def test_a_script_with_no_expect_file_fails(self):
        self.remove("systems/demo.expect")
        self.remove("systems/rpi5b_2gb/demo.expect")
        self.assert_fails("no expect file of demo forbids what it types")

    def test_an_escape_qemu_test_refuses_fails(self):
        self.write("systems/demo.qmp", SCRIPT.replace("\\r", "\\q"))
        self.assert_fails("an unknown escape")

    def test_a_bare_escape_before_text_fails(self):
        # The terminal decoder would swallow the z, and type "qvjsmith".
        self.write("systems/demo.qmp", SCRIPT.replace("serial zqvj", "serial \\ezqvjsmith"))
        self.assert_fails("an ESC before 'z'")

    def test_a_bare_escape_and_an_intermediate_before_text_fails(self):
        self.write("systems/demo.qmp", SCRIPT.replace("serial zqvj", "serial \\e\\x20zqvj"))
        self.assert_fails("an ESC before ' '")

    def test_a_bare_escape_ending_a_step_before_text_fails(self):
        self.write("systems/demo.qmp", SCRIPT.replace("serial zqvj", "serial \\t\\e\nserial zqvj"))
        self.assert_fails("demo.qmp:3: an ESC before 'z'")

    def test_a_sequence_s_final_byte_is_not_text(self):
        # ESC [ z is a sequence of no key: the board types "qvj".
        self.write("systems/demo.qmp", SCRIPT.replace("serial zqvj", "serial \\e[zqvj"))
        self.assert_fails("types 'qvj'")

    def test_an_indented_step_is_read(self):
        self.write("systems/demo.qmp", SCRIPT.replace("serial zqvj", "    serial   smith"))
        self.assert_fails("types 'smith'")

    def test_keys_are_not_text(self):
        runs = check_quiet.typed_runs
        data = check_quiet.serial_bytes
        self.assertEqual(runs(data("\\e[B\\t\\x20\\e[Z\\r\\eOA\\e[1;5C")), [])
        self.assertEqual(runs(data("zqvjb\\t\\t\\r")), ["zqvjb"])
        # Escape itself, then a key or another ESC: nothing swallowed.
        self.assertEqual(runs(data("\\e\\tzqvj\\e\\e[Bzqvjc\\e")), ["zqvj", "zqvjc"])
        # A sequence cut short by a control byte: the byte is read afresh.
        self.assertEqual(runs(data("\\e[1\\tzqvj")), ["zqvj"])


if __name__ == "__main__":
    unittest.main()
