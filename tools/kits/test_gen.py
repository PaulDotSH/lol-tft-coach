import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import gen

ROOT = Path(__file__).resolve().parents[2]
BIN = ROOT / "fixtures" / "data" / "cdragon-ahri-16_21.bin.json"

AHRI = {
    "id": "Ahri",
    "stats": {"attackdamage": 53, "attackdamageperlevel": 0},
    "spells": [
        {"maxrank": 5, "cooldown": [7, 7, 7, 7, 7], "tooltip": "dealing <magicDamage>{{ totaldamage }} magic damage</magicDamage> on the way out and <trueDamage>{{ totaldamage }} true damage</trueDamage> on the way back."},
        {"maxrank": 5, "cooldown": [9, 8, 7, 6, 5], "tooltip": "deal <magicDamage>{{ singlefiredamage }} magic damage</magicDamage>, reduced to <magicDamage>{{ multifiredamage }} damage</magicDamage> beyond the first. <speed>{{ movementspeed*100 }}% Move Speed</speed>"},
        {"maxrank": 5, "cooldown": [12, 12, 12, 12, 12], "tooltip": "deals <magicDamage>{{ totaldamage }} magic damage</magicDamage>."},
        {"maxrank": 3, "cooldown": [140, 120, 100], "tooltip": "These bolts deal <magicDamage>{{ rcalculateddamage }} magic damage</magicDamage> each."},
    ],
}


class AhriBin(unittest.TestCase):
    def setUp(self):
        self.kit = gen.parse_champion(json.loads(BIN.read_text(encoding="utf-8")), AHRI)

    def test_base_ad_from_root_record(self):
        self.assertEqual((self.kit["base_ad"], self.kit["ad_per_level"]), (53.0, 3.0))

    def test_q_is_magic_out_and_true_back(self):
        q = [s for s in self.kit["spells"] if s["key"] == "Q"]
        self.assertEqual([s["kind"] for s in q], ["magic", "true"])
        self.assertEqual(q[0]["base"], [35.0, 60.0, 85.0, 110.0, 135.0])
        self.assertAlmostEqual(q[0]["ap"][0], 0.5)
        self.assertEqual(q[0]["cooldown"], [7.0] * 5)

    def test_first_value_of_a_kind_only(self):
        w = [s for s in self.kit["spells"] if s["key"] == "W"]
        self.assertEqual(len(w), 1)

    def test_r_has_three_ranks(self):
        r = [s for s in self.kit["spells"] if s["key"] == "R"][0]
        self.assertEqual((len(r["base"]), r["cooldown"]), (3, [140.0, 120.0, 100.0]))


class Slots(unittest.TestCase):
    def test_damage_slots_skip_non_damage(self):
        self.assertEqual(gen.damage_slots(AHRI["spells"][1]["tooltip"]), [("magic", "singlefiredamage", 1.0)])


if __name__ == "__main__":
    unittest.main()
