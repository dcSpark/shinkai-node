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

if __name__ == '__main__':
    unittest.main()