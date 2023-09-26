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
import { useCreateAgent } from "../api/mutations/createAgent/useCreateAgent";
import SimpleLayout from "./layout/simple-layout";
import { useNavigate } from "react-router-dom";
import { CREATE_JOB_PATH } from "../routes/name";
import ErrorMessage from "../components/ui/error-message";

const addAgentSchema = z.object({
  agentName: z.string(),
  externalUrl: z.string().url(),
  performLocally: z.boolean(),
  apikey: z.string(),
  model: z.string(),
});

const CreateAgentPage = () => {
  const { setupData } = useAuth();
  const navigate = useNavigate();
  const addAgentForm = useForm<z.infer<typeof addAgentSchema>>({
    resolver: zodResolver(addAgentSchema),
  });
  const {
    mutateAsync: createAgent,
    isLoading,
    isError,
    error,
  } = useCreateAgent({
    onSuccess: () => {
      navigate(CREATE_JOB_PATH);
    },
  });

  const onSubmit = async (data: z.infer<typeof addAgentSchema>) => {
    createAgent({
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
  };

  return (
    <SimpleLayout title="Add Agent AI">
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
                    <Input placeholder="Eg: https://api.openai.com" {...field} />
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
                    <Input {...field} placeholder="Eg: xYz1DFa..." />
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
                        <SelectValue placeholder="Select your Model" />
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

          {isError && <ErrorMessage message={error.message} />}

          <Button
            className="w-full"
            type="submit"
            isLoading={isLoading}
            disabled={isLoading}
          >
            Create Agent
          </Button>
        </form>
      </Form>
    </SimpleLayout>
  );
};
export default CreateAgentPage;
