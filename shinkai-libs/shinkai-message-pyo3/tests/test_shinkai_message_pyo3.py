import unittest
import shinkai_message_pyo3

class TestShinkaiMessagePyO3(unittest.TestCase):
    def test_shinkai_message(self):
        # Create a ShinkaiMessage object
        message = shinkai_message_pyo3.PyShinkaiMessage()

        # Set the fields of the message
        message.body = shinkai_message_pyo3.PyMessageBody()
        message.external_metadata = shinkai_message_pyo3.PyExternalMetadata()
        message.encryption = shinkai_message_pyo3.PyEncryptionMethod()
        message.version = shinkai_message_pyo3.PyShinkaiVersion()

        # Test that the fields were set correctly
        self.assertIsInstance(message.body, shinkai_message_pyo3.PyMessageBody)
        self.assertIsInstance(message.external_metadata, shinkai_message_pyo3.PyExternalMetadata)
        self.assertIsInstance(message.encryption, shinkai_message_pyo3.PyEncryptionMethod)
        self.assertIsInstance(message.version, shinkai_message_pyo3.PyShinkaiVersion)

         # Print a message to stdout
        print("All tests passed for shinkai_message_pyo3!")

    def test_terminate_message(self):
        # Define some test inputs
        my_encryption_secret_key = "test_encryption_key"
        my_signature_secret_key = "test_signature_key"
        receiver_public_key = "test_receiver_key"
        sender = "test_sender"
        receiver = "test_receiver"

        # Call terminate_message with the test inputs
        result = shinkai_message_pyo3.terminate_message(
            my_encryption_secret_key,
            my_signature_secret_key,
            receiver_public_key,
            sender,
            receiver,
        )

        # Check that terminate_message returned a string
        self.assertIsInstance(result, str)

        # Optionally, if you know what the output should look like, you can check that the output is correct
        # For example, if you know that the output should be a JSON string, you can parse it and check its contents
        # result_json = json.loads(result)
        # self.assertEqual(result_json["some_field"], "some_value")

if __name__ == '__main__':
    unittest.main()