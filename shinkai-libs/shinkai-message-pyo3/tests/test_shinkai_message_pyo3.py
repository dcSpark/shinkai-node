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

    def test_initial_registration_with_no_code_for_device(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_encryption_pk_string = '5b0d4a7f7135ebe6712a65256b9bcb2cf79ee7425407da3cbb51f07dd9d68235'

        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        my_identity_pk_string = '4e91b8ca811cdb07c636190e3f1bc39edcf8ac47cfd4d1c3267fec3be570e740'
        node = "@@node1.shinkai"

        # _registration_with_no_code_for_device
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.initial_registration_with_no_code_for_device( 
            my_encryption_sk_string,  # device
            my_identity_sk_string,  # device
            my_encryption_sk_string,  # profile
            my_identity_sk_string,  # profile
            "main_device",
            node,
            "",
            node,
        )

        # print("Result of initial registration no code:", result)

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_raw_content"], '{"code":"","registration_name":"main_device","device_identity_pk":"4e91b8ca811cdb07c636190e3f1bc39edcf8ac47cfd4d1c3267fec3be570e740","device_encryption_pk":"5b0d4a7f7135ebe6712a65256b9bcb2cf79ee7425407da3cbb51f07dd9d68235","profile_identity_pk":"4e91b8ca811cdb07c636190e3f1bc39edcf8ac47cfd4d1c3267fec3be570e740","profile_encryption_pk":"5b0d4a7f7135ebe6712a65256b9bcb2cf79ee7425407da3cbb51f07dd9d68235","identity_type":"device","permission_type":"admin"}')
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_content_schema"], "UseRegistrationCode")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["sender_subidentity"], "")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["recipient_subidentity"], "")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["inbox"], "inbox::@@node1.shinkai::@@node1.shinkai::false")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["encryption"], "None")
        self.assertEqual(result_json["external_metadata"]["sender"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@node1.shinkai")
        self.assertEqual(result_json["encryption"], "None")
        self.assertEqual(result_json["version"], "V1_0")

if __name__ == '__main__':
    unittest.main()