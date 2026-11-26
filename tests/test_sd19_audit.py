import importlib.util
import io
from pathlib import Path
import unittest
from zipfile import ZipFile

SCRIPT = Path(__file__).resolve().parents[1] / "scripts/audit-sd19-writers.py"
SPEC = importlib.util.spec_from_file_location("sd19_audit", SCRIPT)
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


def entries(part, writer, count, form="14"):
    return [
        f"by_write/hsf_{part}/f{writer}_{form}/d{writer}_{form}/d{writer}_{form}_{index:05}.png"
        for index in range(count)
    ]


def run(paths):
    buffer = io.BytesIO()
    with ZipFile(buffer, "w") as archive:
        for path in paths:
            # Not an image. Inventory must use names without inspecting payloads.
            archive.writestr(path, b"fixture")
    with ZipFile(buffer) as archive:
        return audit.inventory(archive)


class InventoryTest(unittest.TestCase):
    def test_groups_forms_and_ignores_non_digit_payloads(self):
        data = entries(0, "0001", 32)
        data += entries(2, "1001", 16) + entries(2, "1001", 16, form="15")
        data += entries(2, "1002", 31) + entries(4, "2100", 32)
        data += ["by_write/hsf_2/f1003_14/u1003_14/u1003_14_00000.png"]
        report = run(data)
        self.assertEqual(report, run(reversed(data)))
        self.assertEqual(report["partitions"]["hsf_2"], {"writer_ids": 2, "eligible_writer_ids": 1})
        expected_role = audit.role(2, "1001")
        self.assertEqual(report["eligible_writers_by_role"][expected_role], 1)
        self.assertEqual(report["eligible_writers_by_role"]["development"], 1)
        self.assertEqual(report["eligible_writers_by_role"]["shift"], 1)
        self.assertFalse(report["inventory_floor_met"])

    def test_writer_cannot_cross_partition_even_with_another_form(self):
        with self.assertRaisesRegex(ValueError, "multiple partitions"):
            run(entries(2, "1001", 1) + entries(3, "1001", 1, form="15"))

    def test_wrong_archive_refused(self):
        with self.assertRaisesRegex(ValueError, "no SD19"):
            run(["by_class/30/example.png"])

    def test_hash_roles_have_a_stable_literal_example(self):
        self.assertEqual(audit.role(2, "1001"), "calibration")
        self.assertEqual(audit.role(3, "1501"), "audit")


if __name__ == "__main__":
    unittest.main()
