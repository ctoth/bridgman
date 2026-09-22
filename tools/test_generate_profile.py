import unittest

import yaml

from generate_profile import SOURCE, render, validate


class ProfileTests(unittest.TestCase):
    def setUp(self):
        self.profile = yaml.safe_load(SOURCE.read_text(encoding="utf-8"))

    def test_bad_product_dimensions_are_rejected(self):
        self.profile["rules"][0][3] = "Energy"
        with self.assertRaisesRegex(ValueError, "dimensionally invalid"):
            validate(self.profile)

    def test_root_degree_is_not_silently_ignored(self):
        self.profile["roots"][0]["degree"] = 3
        with self.assertRaisesRegex(ValueError, "square root"):
            validate(self.profile)

    def test_multiple_roots_share_one_dispatcher(self):
        self.profile["roots"].append(
            {"kind": "Unitless", "degree": 2, "result": "Unitless", "negative_error": "NegativeRoot"}
        )
        output = render(self.profile)["profile_operations.rs"]
        self.assertEqual(output.count("fn sqrt_dynamic("), 1)
        self.assertIn("Kind::Unitless =>", output)

    def test_bound_precision_is_preserved(self):
        self.profile["bounds"][0]["lower"] = 0.125
        self.assertIn("value < 0.125", render(self.profile)["profile_operations.rs"])

    def test_difference_kind_cannot_be_a_point(self):
        self.profile["affine_spaces"].append({"point": "TemperatureDelta", "difference": "Temperature"})
        with self.assertRaisesRegex(ValueError, "affine"):
            validate(self.profile)

    def test_linearity_follows_affine_spaces(self):
        output = render(self.profile)["profile_kinds.rs"]
        self.assertNotIn("Temperature,", output.split("linear!(")[1])
        self.assertIn("TemperatureDelta", output.split("linear!(")[1])

    def test_offsets_require_a_point_kind(self):
        self.profile["units"][0][5] = 1
        with self.assertRaisesRegex(ValueError, "offsets"):
            validate(self.profile)

    def test_unicode_unit_symbols_are_valid_rust_literals(self):
        self.profile["units"][3][2] = "°C"
        self.assertIn('"°C"', render(self.profile)["profile_units.rs"])


if __name__ == "__main__":
    unittest.main()
