import unittest
import shinkai_message_pyo3
import json

class TestShinkaiMessagePyO3(unittest.TestCase):
    def test_ack_message(self):
        my_encryption_secret_key = "d83f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81159"
        my_signature_secret_key = "df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119"
        receiver_public_key = "798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679"
        sender = "@@sender.shinkai"
        receiver = "@@receiver.shinkai"

        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.ack_message(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            receiver
        )

        # print("Result of ack_message:", result)

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_raw_content"], "ACK")
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_content_schema"], "Empty")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["sender_subidentity"], "")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["recipient_subidentity"], "")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["inbox"], "inbox::@@receiver.shinkai::@@sender.shinkai::false")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["encryption"], "None")
        self.assertEqual(result_json["external_metadata"]["sender"], "@@sender.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@receiver.shinkai")
        self.assertEqual(result_json["encryption"], "None")
        self.assertEqual(result_json["version"], "V1_0")

if __name__ == '__main__':
    unittest.main()