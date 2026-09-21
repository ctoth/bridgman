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

    def test_point_role_cannot_be_linear(self):
        self.profile["kinds"][1]["linear"] = True
        with self.assertRaisesRegex(ValueError, "affine"):
            validate(self.profile)


if __name__ == "__main__":
    unittest.main()
