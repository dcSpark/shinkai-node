import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { PlusIcon } from "lucide-react";
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
import { ADD_AGENT_PATH } from "../routes/name";
import { useAuth } from "../store/auth";
import SimpleLayout from "./layout/simple-layout";

const createJobSchema = z.object({
  model: z.string(),
  description: z.string(),
});

const CreateJobPage = () => {
  const auth = useAuth((state) => state.auth);
  const navigate = useNavigate();

  const { agents } = useAgents({
    sender: auth?.shinkai_identity ?? "",
    senderSubidentity: `${auth?.profile}`,
    shinkaiIdentity: auth?.shinkai_identity ?? "",
    my_device_encryption_sk: auth?.profile_encryption_sk ?? "",
    my_device_identity_sk: auth?.profile_identity_sk ?? "",
    node_encryption_pk: auth?.node_encryption_pk ?? "",
    profile_encryption_sk: auth?.profile_encryption_sk ?? "",
    profile_identity_sk: auth?.profile_identity_sk ?? "",
  });

  const { isPending, mutateAsync: createJob } = useCreateJob({
    onSuccess: (data) => {
      // TODO: job_inbox, false is hardcoded
      navigate(`/inboxes/job_inbox::${data.jobId}::false`);
    },
  });

  const createJobForm = useForm<z.infer<typeof createJobSchema>>({
    resolver: zodResolver(createJobSchema),
  });

  const onSubmit = async (data: z.infer<typeof createJobSchema>) => {
    if (!auth) return;
    createJob({
      shinkaiIdentity: auth.shinkai_identity,
      profile: auth.profile,
      agentId: data.model,
      content: data.description,
      files_inbox: "",
      files: [],
      my_device_encryption_sk: auth.my_device_encryption_sk,
      my_device_identity_sk: auth.my_device_identity_sk,
      node_encryption_pk: auth.node_encryption_pk,
      profile_encryption_sk: auth.profile_encryption_sk,
      profile_identity_sk: auth.profile_identity_sk,
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
                      {agents?.length ? (
                        agents.map((agent) => (
                          <SelectItem key={agent.id} value={agent.id}>
                            <span>{agent.id} </span>
                          </SelectItem>
                        ))
                      ) : (
                        <Button
                          onClick={() => {
                            navigate(ADD_AGENT_PATH);
                          }}
                          variant="ghost"
                        >
                          <PlusIcon className="mr-2" />
                          Add Agents
                        </Button>
                      )}
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
            disabled={isPending}
            isLoading={isPending}
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
