import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";

import { useCreateJob } from "../api/mutations/createJob/useCreateJob";
import { useAgents } from "../api/queries/getAgents/useGetAgents";
import { Button } from "../components/ui/button";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "../components/ui/form";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select";
import { Textarea } from "../components/ui/textarea";
import { useAuth } from "../store/auth-context";
import SimpleLayout from "./layout/simple-layout";

const createJobSchema = z.object({
  model: z.string(),
  description: z.string(),
});

const CreateJobPage = () => {
  const { setupData } = useAuth();
  const navigate = useNavigate();

  const { agents } = useAgents({
    sender: setupData?.shinkai_identity ?? "",
    senderSubidentity: `${setupData?.profile}`,
    shinkaiIdentity: setupData?.shinkai_identity ?? "",
    my_device_encryption_sk: setupData?.profile_encryption_sk ?? "",
    my_device_identity_sk: setupData?.profile_identity_sk ?? "",
    node_encryption_pk: setupData?.node_encryption_pk ?? "",
    profile_encryption_sk: setupData?.profile_encryption_sk ?? "",
    profile_identity_sk: setupData?.profile_identity_sk ?? "",
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
    if (!setupData) return;
    createJob({
      shinkaiIdentity: setupData.shinkai_identity,
      profile: setupData.profile,
      agentId: data.model,
      content: data.description,
      files_inbox: "",
      my_device_encryption_sk: setupData.my_device_encryption_sk,
      my_device_identity_sk: setupData.my_device_identity_sk,
      node_encryption_pk: setupData.node_encryption_pk,
      profile_encryption_sk: setupData.profile_encryption_sk,
      profile_identity_sk: setupData.profile_identity_sk,
    });
  };
  return (
    <SimpleLayout title="Create Job">
      <Form {...createJobForm}>
        <form className="space-y-10" onSubmit={createJobForm.handleSubmit(onSubmit)}>
          <div className="space-y-6">
            <FormField
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Select your AI Agent</FormLabel>
                  <Select defaultValue={field.value} onValueChange={field.onChange}>
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
              control={createJobForm.control}
              name="model"
            />
            <FormField
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Tell us the job you want to do</FormLabel>
                  <FormControl>
                    <Textarea
                      className="resize-none border-white"
                      placeholder="Eg: Explain me how internet works..."
                      {...field}
                    />
                  </FormControl>
                  <FormMessage />
                </FormItem>
              )}
              control={createJobForm.control}
              name="description"
            />
          </div>

          <Button
            className="w-full"
            disabled={isLoading}
            isLoading={isLoading}
            type="submit"
          >
            Create Job
          </Button>
        </form>
      </Form>
    </SimpleLayout>
  );
};
export default CreateJobPage;
