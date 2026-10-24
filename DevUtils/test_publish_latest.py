import unittest
from publish_latest import promote_assets


class PromotionTests(unittest.TestCase):
    def test_failed_promotion_restores_old_downloads_without_deleting_them(self):
        names = {1: "game.zip", 2: "game.sha256", 3: "new.zip.uploading", 4: "new.sha.uploading"}
        original = names.copy()
        calls = []
        def request(path, method, body=None):
            identifier = int(path.rsplit("/", 1)[1])
            calls.append(method)
            if identifier == 4 and body and body["name"] == "game.sha256":
                raise RuntimeError("simulated failure")
            names[identifier] = body["name"]
        with self.assertRaises(RuntimeError):
            promote_assets([{"id": i, "name": original[i]} for i in [1, 2]],
                           [({"id": 3, "name": original[3]}, "game.zip"),
                            ({"id": 4, "name": original[4]}, "game.sha256")],
                           lambda: self.fail("must not publish after failure"), request)
        self.assertEqual(names, original)
        self.assertNotIn("DELETE", calls)

    def test_old_assets_deleted_only_after_release_is_published(self):
        calls = []
        def request(path, method, body=None):
            calls.append(method)
        promote_assets([{"id": 1, "name": "game.zip"}],
                       [({"id": 2, "name": "new.uploading"}, "game.zip")],
                       lambda: calls.append("PUBLISH"), request)
        self.assertEqual(calls, ["PATCH", "PATCH", "PUBLISH", "DELETE"])

    def test_failed_release_metadata_restores_old_names(self):
        names = {1: "game.zip", 2: "new.uploading"}
        def request(path, method, body=None):
            self.assertEqual(method, "PATCH")
            names[int(path.rsplit("/", 1)[1])] = body["name"]
        def fail():
            raise RuntimeError("metadata failed")
        with self.assertRaises(RuntimeError):
            promote_assets([{"id": 1, "name": "game.zip"}],
                           [({"id": 2, "name": "new.uploading"}, "game.zip")], fail, request)
        self.assertEqual(names, {1: "game.zip", 2: "new.uploading"})
