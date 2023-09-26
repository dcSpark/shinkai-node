import { z } from "zod";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "../components/ui/form";
import SimpleLayout from "./layout/simple-layout";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { Button } from "../components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Textarea } from "../components/ui/textarea";
import { useAgents } from "../api/queries/getAgents/useGetAgents";
import { useAuth } from "../store/auth-context";
import { useCreateJob } from "../api/mutations/createJob/useCreateJob";
import { useNavigate } from "react-router-dom";

const createJobSchema = z.object({
  model: z.string(),
  description: z.string(),
});

const CreateJobPage = () => {
  const { setupData } = useAuth();
  const navigate = useNavigate();

  const { agents } = useAgents({
    sender: setupData?.shinkai_identity ?? "",
    senderSubidentity: `${setupData?.profile}/device/${setupData?.registration_name}`,
    shinkaiIdentity: setupData?.shinkai_identity ?? "",
    my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
    my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
    node_encryption_pk: setupData?.node_encryption_pk ?? "",
  });

  const { isLoading, mutateAsync: createJob } = useCreateJob({
    onSuccess: (data) => {
      // TODO: job_inbox, false is hardcoded
      navigate(`/inboxes/job_inbox::${data.jobId}::false`);
    },
  });

  const createJobForm = useForm<z.infer<typeof createJobSchema>>({
    resolver: zodResolver(createJobSchema),
  });

  const onSubmit = async (data: z.infer<typeof createJobSchema>) => {
    createJob({
      shinkaiIdentity: setupData?.shinkai_identity ?? "",
      profile: setupData?.profile ?? "",
      agentId: data.model,
      content: data.description,
      my_device_encryption_sk: setupData?.my_device_encryption_sk ?? "",
      my_device_identity_sk: setupData?.my_device_identity_sk ?? "",
      node_encryption_pk: setupData?.node_encryption_pk ?? "",
      profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
      profile_identity_sk: setupData?.profile_identity_sk ?? "",
    });
  };
  return (
    <SimpleLayout title="Create Job">
      <Form {...createJobForm}>
        <form className="space-y-10" onSubmit={createJobForm.handleSubmit(onSubmit)}>
          <div className="space-y-6">
            <FormField
              control={createJobForm.control}
              name="model"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Select your AI Agent</FormLabel>
                  <Select onValueChange={field.onChange} defaultValue={field.value}>
                    <FormControl>
                      <SelectTrigger>
                        <SelectValue placeholder="Select your AI Agent" />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                      {agents?.map((agent) => (
                        <SelectItem key={agent.id} value={agent.id}>
                          <span>{agent.id} </span>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FormItem>
              )}
            />
            <FormField
              control={createJobForm.control}
              name="description"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Tell us the job you want to do</FormLabel>
                  <FormControl>
                    <Textarea
                      placeholder="Eg: Explain me how internet works..."
                      className="resize-none border-white"
                      {...field}
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
            />
          </div>

          <Button
            className="w-full"
            type="submit"
            isLoading={isLoading}
            disabled={isLoading}
          >
            Create Job
          </Button>
        </form>
      </Form>
    </SimpleLayout>
  );
};
export default CreateJobPage;
