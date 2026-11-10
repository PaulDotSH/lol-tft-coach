"""Synthetic tests for autolabel.py (v2) and the head export. py -3.12 -I tools/tft-render/test_autolabel.py"""
import os
import sys
import tempfile
import unittest

import numpy as np

sys.path.insert(0, os.path.dirname(__file__))
import autolabel as A  # noqa: E402

UNITS = ['DA_18_Ahri', 'DA_18_Lux', 'DA_18_Sett', 'DA_18_Vi', 'DA_18_Zed']


def looks(apis, sure=6.0):
    """Log-probabilities of crops that look like `apis` (each clearly its own unit)."""
    z = np.zeros((len(apis), len(UNITS)))
    for i, a in enumerate(apis):
        z[i, UNITS.index(a)] = sure
    return z - np.log(np.exp(z).sum(1, keepdims=True))


def crops(stars):
    return [{'x': 100.0 * i, 'y': 200.0, 'star': s} for i, s in enumerate(stars)]


BOARD = [('DA_18_Ahri', 2), ('DA_18_Lux', 1), ('DA_18_Sett', 3)]
PLAYER = {'name': 'LeoDp', 'units': BOARD, 'last': 30}


class Autolabel(unittest.TestCase):
    def test_final_board_crops_label_every_crop(self):
        apis = ['DA_18_Sett', 'DA_18_Ahri', 'DA_18_Lux']
        got = A.label_fight((5, 5), crops([3, 2, 1]), PLAYER, looks(apis), UNITS)
        self.assertEqual(got, apis)

    def test_outside_label_rounds_labels_none(self):
        rnd = A.round_index(5, 5)
        early = dict(PLAYER, last=rnd + A.LABEL_ROUNDS + 1)
        self.assertEqual(A.label_fight((5, 5), crops([2]), early, looks(['DA_18_Ahri']), UNITS), [None])
        self.assertEqual(A.label_fight((5, 5), crops([2]), None, looks(['DA_18_Ahri']), UNITS), [None])

    def test_star_constraints_hold(self):
        lp = looks(['DA_18_Ahri', 'DA_18_Lux'])
        self.assertEqual(A.assign(lp, [3, 2], BOARD, UNITS), [None, None])
        lp = looks(['DA_18_Sett', 'DA_18_Ahri'], sure=3.0)
        self.assertEqual(A.assign(lp, [3, 2], BOARD, UNITS), ['DA_18_Sett', 'DA_18_Ahri'])
        self.assertEqual(A.assign(looks(['DA_18_Ahri']), [0], BOARD, UNITS), ['DA_18_Ahri'])

    def test_one_slot_takes_one_crop(self):
        got = A.assign(looks(['DA_18_Lux', 'DA_18_Lux']), [1, 1], BOARD, UNITS)
        self.assertEqual(sorted(got, key=str), ['DA_18_Lux', None])

    def test_units_off_the_board_and_unsure_crops_stay_unlabelled(self):
        self.assertEqual(A.assign(looks(['DA_18_Zed']), [1], BOARD, UNITS), [None])
        z = np.log(np.full((1, len(UNITS)), 1 / len(UNITS)))
        self.assertEqual(A.assign(z, [1], BOARD, UNITS), [None])

    def test_link_by_plate_candidates_and_fit(self):
        players = [PLAYER, {'name': 'Kabeas', 'units': [('DA_18_Vi', 2), ('DA_18_Zed', 1)], 'last': 25}, {'name': 'Gone', 'units': BOARD, 'last': 10}]
        lp = looks(['DA_18_Vi', 'DA_18_Zed'])
        self.assertEqual(A.link((4, 2), 'LeoDp', {}, players, lp, UNITS), (PLAYER, 'plate'))
        self.assertEqual(A.link((4, 2), None, {(4, 2): {'opponent': None, 'candidates': ['Kabeas']}}, players, None, UNITS), (players[1], 'hp'))
        self.assertEqual(A.link((4, 2), None, {}, players, lp, UNITS), (players[1], 'fit'))
        self.assertEqual(A.link((4, 2), 'Gone', {}, players, None, UNITS), (None, None))
        self.assertEqual(A.link((4, 2), None, {}, players[:1] + players[2:], None, UNITS), (PLAYER, 'hp'))

    def test_truth_needs_a_clear_majority(self):
        players = [PLAYER, {'name': 'Kabeas', 'units': [('DA_18_Vi', 2), ('DA_18_Ahri', 1)], 'last': 30}]
        self.assertIs(A.truth((4, 2), ['DA_18_Lux', 'DA_18_Sett'], players), PLAYER)
        self.assertIsNone(A.truth((4, 2), ['DA_18_Ahri'], players))
        self.assertIsNone(A.truth((4, 2), [], players))

    def test_game_folders_of_both_writers(self):
        self.assertEqual(A.game_start('1794000000'), 1794000000)
        self.assertIsNotNone(A.game_start('20261025-024840'))
        self.assertIsNone(A.game_start('notes'))


class HeadExport(unittest.TestCase):
    def test_export_reads_back_and_matches_torch(self):
        import torch
        import head as H
        m, units = H.tiny()
        with tempfile.TemporaryDirectory() as d:
            p = os.path.join(d, 'h.bin')
            H.export(m, units, 0.7, 0.2, p)
            dd, hidden, names, sim, margin, (w1, b1, w2, b2) = H.read_head(p)
        self.assertEqual((dd, hidden, names, round(sim, 5), round(margin, 5)), (12, 8, units, 0.7, 0.2))
        x = np.random.default_rng(3).normal(size=(20, 12)).astype(np.float32)
        h = torch.nn.functional.gelu(torch.tensor(x @ w1.reshape(8, 12).T + b1))
        mine = torch.softmax(h @ torch.tensor(w2.reshape(-1, 8).T) + torch.tensor(b2), -1).numpy()
        with torch.no_grad():
            want = torch.softmax(m(torch.tensor(x)), -1).numpy()
        self.assertTrue(np.abs(mine - want).max() < 5e-3)


if __name__ == '__main__':
    unittest.main()
