import json
import unittest
import shinkai_message_pyo3


class TestPyLLMProviderInterface(unittest.TestCase):
    def test_new_openai(self):
        model_type = "gpt-3.5-turbo-1106"
        agent_llm_interface = shinkai_message_pyo3.PyLLMProviderInterface.new_openai(model_type)
        self.assertEqual(agent_llm_interface.get_model(), "openai:" + model_type)

    def test_new_openai(self):
        model_type = "mistralai/Mistral-7B-Instruct-v0.1"
        agent_llm_interface = shinkai_message_pyo3.PyLLMProviderInterface.new_genericapi(model_type)
        self.assertEqual(agent_llm_interface.get_model(), "genericapi:" + model_type)

    def test_new_localllm(self):
        agent_llm_interface = shinkai_message_pyo3.PyLLMProviderInterface.new_localllm()
        self.assertEqual(agent_llm_interface.get_model(), "LocalLLM")

    def test_new_with_openai_prefix(self):
        model_type = "gpt-3.5-turbo-1106"
        agent_llm_interface = shinkai_message_pyo3.PyLLMProviderInterface("openai:" + model_type)
        self.assertEqual(agent_llm_interface.get_model(), "openai:" + model_type)

    def test_new_with_genericapi_prefix(self):
        model_type = "mistralai/Mistral-7B-Instruct-v0.1"
        agent_llm_interface = shinkai_message_pyo3.PyLLMProviderInterface("genericapi:" + model_type)
        self.assertEqual(agent_llm_interface.get_model(), "genericapi:" + model_type)

    def test_new_without_openai_prefix(self):
        agent_llm_interface = shinkai_message_pyo3.PyLLMProviderInterface("not_openai")
        self.assertEqual(agent_llm_interface.get_model(), "LocalLLM")


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

    def test_job_creation(self):        
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        node = "@@node1.shinkai"

        # job_scope
        job_scope = shinkai_message_pyo3.PyJobScope()

        # job_creation
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.job_creation( 
            my_encryption_sk_string,  # device
            my_identity_sk_string,  # device
            my_encryption_sk_string,  # profile
            job_scope, 
            False,
            node,
            "main",
            node,
            "main/agent/agent_1",
        )

        # print("Result:", result)

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        # self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_raw_content"], "{\"scope\":{\"local\":[],\"vector_fs_items\":[],\"vector_fs_folders\":[],\"network_folders\":[]},\"is_hidden\":false}")
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_content_schema"], "JobCreationSchema")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["sender_subidentity"], "main")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["recipient_subidentity"], "main/agent/agent_1")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["inbox"], "inbox::@@node1.shinkai/main::@@node1.shinkai/main/agent/agent_1::false")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["encryption"], "None")
        self.assertEqual(result_json["external_metadata"]["sender"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@node1.shinkai")
        self.assertEqual(result_json["encryption"], "None")
        self.assertEqual(result_json["version"], "V1_0")
    
    def test_job_message(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        node = "@@node1.shinkai"
        job_id = "job1"
        content = "Job content"
        files_inbox = ""

        # job_message
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.job_message(
            job_id,
            content,
            files_inbox,
            '',
            my_encryption_sk_string,
            my_identity_sk_string,
            my_encryption_sk_string,
            node,
            '',
            node,
            "main/agent/agent_1",
            None,
            None,
        )

        # print("Result:", result)

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_raw_content"], "{\"job_id\":\"job1\",\"content\":\"Job content\",\"files_inbox\":\"\",\"parent\":\"\",\"workflow_code\":null,\"workflow_name\":null,\"sheet_job_data\":null,\"callback\":null}")
        self.assertEqual(result_json["body"]["unencrypted"]["message_data"]["unencrypted"]["message_content_schema"], "JobMessageSchema")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["sender_subidentity"], "")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["recipient_subidentity"], "main/agent/agent_1")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["inbox"], "job_inbox::job1::false")
        self.assertEqual(result_json["body"]["unencrypted"]["internal_metadata"]["encryption"], "None")
        self.assertEqual(result_json["external_metadata"]["sender"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@node1.shinkai")
        self.assertEqual(result_json["encryption"], "None")
        self.assertEqual(result_json["version"], "V1_0")

    def test_get_last_unread_messages_from_inbox(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        node = "@@node1.shinkai"
        inbox = "job_inbox::job1::false"
        count = 10
        offset = None

        # get_last_unread_messages_from_inbox
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.get_last_unread_messages_from_inbox(
            my_encryption_sk_string,
            my_identity_sk_string,
            my_encryption_sk_string,
            inbox,
            count,
            node,
            "main",
            node,
            "main/agent/agent_1",
            offset,
        )
        # print("Result:", result)

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["intra_sender"], "main")
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")

    def test_get_last_messages_from_inbox(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        node = "@@node1.shinkai"
        inbox = "job_inbox::job1::false"
        count = 10
        offset = None

        # get_last_messages_from_inbox
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.get_last_messages_from_inbox(
            my_encryption_sk_string,
            my_identity_sk_string,
            my_encryption_sk_string,
            inbox,
            count,
            node,
            "main",
            node,
            "main/agent/agent_1",
            offset,
        )

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["intra_sender"], "main")
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")

    def test_request_add_agent(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        node = "@@node1.shinkai"
        agent = shinkai_message_pyo3.PySerializedLLMProvider.new_with_defaults(
            full_identity_name="@@node1.shinkai",
            id="agent1",
            external_url="http://example.com",
            model="openai:gpt-3.5-turbo-1106",
            api_key="your_api_key_here",
        )

        # Serialize the agent to a JSON string
        agent_json = agent.to_json_str()

        # request_add_agent
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.request_add_agent(
            my_encryption_sk_string,
            my_identity_sk_string,
            my_encryption_sk_string,
            agent_json,
            node,
            "main",
            node,
            "main/agent/agent_1",
        )

        # Parse the result as a JSON object
        result_json = json.loads(result)

        # Add assertions to check the fields of the result
        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["recipient"], "@@node1.shinkai")
        self.assertEqual(result_json["external_metadata"]["intra_sender"], "main")
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")

    def test_create_files_inbox_with_sym_key(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        receiver_public_key = '798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679'
        inbox = "inbox::@@receiver.shinkai::@@sender.shinkai::false"
        symmetric_key_sk = "symmetric_key"
        sender_subidentity = "main"
        sender = "@@sender.shinkai"
        receiver = "@@receiver.shinkai"

        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.create_files_inbox_with_sym_key(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key,
            inbox,
            symmetric_key_sk,
            sender_subidentity,
            sender,
            receiver
        )

        # print("Result:", result)

        result_json = json.loads(result)

        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], sender)
        self.assertEqual(result_json["external_metadata"]["recipient"], receiver)
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")

    def test_get_all_inboxes_for_profile(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        receiver_public_key = '798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679'
        full_profile = "full_profile"
        sender_subidentity = "main"
        sender = "@@sender.shinkai"
        receiver = "@@receiver.shinkai"

        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.get_all_inboxes_for_profile(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key,
            full_profile,
            sender,
            sender_subidentity,
            receiver
        )

        # print("Result:", result)

        result_json = json.loads(result)

        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], sender)
        self.assertEqual(result_json["external_metadata"]["recipient"], receiver)
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")
    
    def test_custom_job_message(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        receiver_public_key = '798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679'
        node = "@@node1.shinkai"
        job_id = "job1"
        content = "Job content"
        files_inbox = ""
        data = json.dumps({
            "job_id": job_id,
            "content": content,
            "files_inbox": files_inbox
        })  # Simulating JobMessage data
        sender = node
        sender_subidentity = ""
        recipient = node
        recipient_subidentity = "main/agent/agent_1"
        other = "job_inbox::job1::false"
        schema = shinkai_message_pyo3.PyMessageSchemaType("JobMessageSchema")

        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.create_custom_shinkai_message_to_node(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key,
            data,
            sender,
            sender_subidentity,
            recipient,
            recipient_subidentity,
            other,
            schema
        )

        # print("Result:", result)
        result_json = json.loads(result)

        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], sender)
        self.assertEqual(result_json["external_metadata"]["recipient"], recipient)
        self.assertEqual(result_json["external_metadata"]["other"], other)
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")
    
    def test_vecfs_copy_item(self):
        my_encryption_sk_string = '7008829b80ae4350cf049e48d8bce4714e216b674fff0bf34f97f7b98d928d3f'
        my_identity_sk_string = 'b6baf0fa268f993c57223d5db96e5e1de776fcb0195ee6137f33de9d8d9dd749'
        receiver_public_key = '798cbd64d78c4a0fba338b2a6349634940dc4e5b601db1029e02c41e0fe05679'
        origin_path = '/origin/path'
        destination_path = '/destination/path'
        sender = "@@sender.shinkai"
        receiver = "@@receiver.shinkai"
        sender_subidentity = "main"
        receiver_subidentity = ""

        # Action
        result = shinkai_message_pyo3.PyShinkaiMessageBuilder.vecfs_copy_item(
            my_encryption_sk_string,
            my_identity_sk_string,
            receiver_public_key,
            origin_path,
            destination_path,
            sender,
            sender_subidentity,
            receiver,
            receiver_subidentity,
        )

        # Assert
        result_json = json.loads(result)
        self.assertTrue("encrypted" in result_json["body"])
        self.assertEqual(result_json["external_metadata"]["sender"], sender)
        self.assertEqual(result_json["external_metadata"]["recipient"], receiver)
        self.assertEqual(result_json["encryption"], "DiffieHellmanChaChaPoly1305")
        self.assertEqual(result_json["version"], "V1_0")
    
    # TODO: add more tests for the remaining functions
    # TODO: decrypt the result and check the fields of the result

if __name__ == '__main__':
    unittest.main()