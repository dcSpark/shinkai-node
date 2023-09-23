import { z } from "zod";
import {
  Form,
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
} from "../components/ui/form";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Checkbox } from "../components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { useAuth } from "../store/auth-context";
import { useMutation } from "@tanstack/react-query";
import { addAgent } from "@shinkai_network/shinkai-message-ts/api";
import {
  SerializedAgent,
  AgentCredentialsPayload,
} from "../shinkai-message-ts/src/models";
const addAgentSchema = z.object({
  agentName: z.string(),
  externalUrl: z.string().url(),
  performLocally: z.boolean(),
  apikey: z.string(),
  model: z.string(),
});

type CreateAgent = {
  sender_subidentity: string;
  node_name: string;
  agent: SerializedAgent;
  setupDetailsState: AgentCredentialsPayload;
};

const createAgent = async (data: CreateAgent) => {
  const { sender_subidentity, node_name, agent, setupDetailsState } = data;
  return await addAgent(sender_subidentity, node_name, agent, setupDetailsState);
};

const useCreateAgent = () => {
  return useMutation({
    mutationFn: createAgent,
  });
};

const AddAgentPage = () => {
  const { setupData } = useAuth();
  const addAgentForm = useForm<z.infer<typeof addAgentSchema>>({
    resolver: zodResolver(addAgentSchema),
  });
  const { mutateAsync: createAgent, ...rest } = useCreateAgent();
  console.log("response", rest);

  const onSubmit = async (data: z.infer<typeof addAgentSchema>) => {
    console.log({
      sender_subidentity: setupData?.profile ?? "",
      node_name: setupData?.shinkai_identity ?? "",
      agent: {
        allowed_message_senders: [],
        api_key: data.apikey,
        external_url: data.externalUrl,
        full_identity_name: `${setupData?.shinkai_identity}/${setupData?.profile}/agent/${data.agentName}`,
        id: data.agentName,
        perform_locally: data.performLocally,
        storage_bucket_permissions: [],
        toolkit_permissions: [],
        model: {
          OpenAI: {
            model_type: data.model,
          },
        },
      },
      setupDetailsState: {
        my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
        my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
        node_encryption_pk: setupData?.node_encryption_pk ?? "",
        profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
        profile_identity_sk: setupData?.profile_identity_sk ?? "",
      },
    });
    createAgent({
      sender_subidentity: setupData?.profile ?? "",
      node_name: setupData?.shinkai_identity ?? "",
      agent: {
        allowed_message_senders: [],
        api_key: data.apikey,
        external_url: data.externalUrl,
        full_identity_name: `${setupData?.shinkai_identity}/${setupData?.profile}/agent/${data.agentName}}`,
        id: data.agentName,
        perform_locally: data.performLocally,
        storage_bucket_permissions: [],
        toolkit_permissions: [],
        model: {
          OpenAI: {
            model_type: data.model,
          },
        },
      },
      setupDetailsState: {
        my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
        my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
        node_encryption_pk: setupData?.node_encryption_pk ?? "",
        profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
        profile_identity_sk: setupData?.profile_identity_sk ?? "",
      },
    });
  };

  return (
    <div className="py-10 max-w-lg mx-auto">
      <h1 className="text-center font-semibold tracking-tight text-2xl mb-8">
        Create AI Agent
      </h1>
      <Form {...addAgentForm}>
        <form className="space-y-10" onSubmit={addAgentForm.handleSubmit(onSubmit)}>
          <div className="space-y-6">
            <FormField
              control={addAgentForm.control}
              name="agentName"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Agent Name</FormLabel>
                  <FormControl>
                    <Input placeholder="Eg: Personal AI Agent" {...field} />
                  </FormControl>
                </FormItem>
              )}
            />
            <FormField
              control={addAgentForm.control}
              name="externalUrl"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>External URL</FormLabel>
                  <FormControl>
                    <Input placeholder="Eg: http://localhost:9550" {...field} />
                  </FormControl>
                </FormItem>
              )}
            />
            <FormField
              control={addAgentForm.control}
              name="performLocally"
              render={({ field }) => (
                <FormItem>
                  <FormControl>
                    <Checkbox checked={field.value} onCheckedChange={field.onChange} />
                  </FormControl>
                  <FormLabel className="ml-3">Perform Locally</FormLabel>
                </FormItem>
              )}
            />
            <FormField
              control={addAgentForm.control}
              name="apikey"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Api Key</FormLabel>
                  <FormControl>
                    <Input {...field} placeholder="Eg: xyz123..." />
                  </FormControl>
                  <FormDescription className="text-xs text-left pt-1">
                    Enter the API key for your agent
                  </FormDescription>
                </FormItem>
              )}
            />

            <FormField
              control={addAgentForm.control}
              name="model"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Select your Model</FormLabel>
                  <Select onValueChange={field.onChange} defaultValue={field.value}>
                    <FormControl>
                      <SelectTrigger>
                        <SelectValue placeholder="Select a verified email to display" />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                      <SelectItem value="gpt-3.5-turbo">OpenAI</SelectItem>
                      <SelectItem value="sleep">SleepAPI</SelectItem>
                    </SelectContent>
                  </Select>
                </FormItem>
              )}
            />
          </div>

          <Button className="w-full" type="submit">
            Create Agent
          </Button>
        </form>
      </Form>
    </div>
  );
};
export default AddAgentPage;
