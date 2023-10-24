import { useEffect } from "react";
import { useDropzone } from "react-dropzone";
import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";

import { zodResolver } from "@hookform/resolvers/zod";
import { ImagePlusIcon, PlusIcon, XCircleIcon } from "lucide-react";
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
import { cn } from "../lib/utils";
import { ADD_AGENT_PATH } from "../routes/name";
import { useAuth } from "../store/auth";
import SimpleLayout from "./layout/simple-layout";

const createJobSchema = z.object({
  model: z.string(),
  description: z.string(),
  file: z.any().optional(),
});

const CreateJobPage = () => {
  const auth = useAuth((state) => state.auth);
  const navigate = useNavigate();

  const createJobForm = useForm<z.infer<typeof createJobSchema>>({
    resolver: zodResolver(createJobSchema),
  });

  const { getRootProps: getRootFileProps, getInputProps: getInputFileProps } =
    useDropzone({
      multiple: false,
      onDrop: (acceptedFiles) => {
        const file = acceptedFiles[0];
        const reader = new FileReader();
        reader.addEventListener("abort", () => console.log("file reading was aborted"));
        reader.addEventListener("load", (event: ProgressEvent<FileReader>) => {
          const binaryUrl = event.target?.result;
          const image = new Image();
          image.addEventListener("load", function () {
            const imageInfo = Object.assign(file, {
              preview: URL.createObjectURL(file),
            });
            createJobForm.setValue("file", imageInfo, { shouldValidate: true });
          });
          image.src = binaryUrl as string;
        });
        reader.readAsDataURL(file);
      },
    });

  const { file } = createJobForm.watch();

  const { agents, isSuccess } = useAgents({
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

  const onSubmit = async (data: z.infer<typeof createJobSchema>) => {
    if (!auth) return;
    createJob({
      shinkaiIdentity: auth.shinkai_identity,
      profile: auth.profile,
      agentId: data.model,
      content: data.description,
      files_inbox: "",
      file: data.file,
      my_device_encryption_sk: auth.my_device_encryption_sk,
      my_device_identity_sk: auth.my_device_identity_sk,
      node_encryption_pk: auth.node_encryption_pk,
      profile_encryption_sk: auth.profile_encryption_sk,
      profile_identity_sk: auth.profile_identity_sk,
    });
  };

  useEffect(
    () => {
      if (isSuccess && agents?.length) {
        createJobForm.setValue("model", agents[0].id);
      }
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [isSuccess]
  );

  return (
    <SimpleLayout title="Create Job">
      <Form {...createJobForm}>
        <form className="space-y-10" onSubmit={createJobForm.handleSubmit(onSubmit)}>
          <div className="space-y-6">
            <FormField
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Select your AI Agent</FormLabel>
                  <Select onValueChange={field.onChange} value={field.value}>
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
                      onKeyDown={(event) => {
                        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                          createJobForm.handleSubmit(onSubmit)();
                        }
                      }}
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

            <div
              {...getRootFileProps({
                className: cn(
                  "dropzone group relative relative flex w-full flex-shrink-0 cursor-pointer items-center justify-center rounded border-2 border-dashed border-slate-500 border-slate-500 transition-colors hover:border-white",
                  file && "border-0"
                ),
              })}
            >
              {!file && (
                <div className="flex flex-col items-center gap-2 p-4">
                  <ImagePlusIcon className="stroke-slate-500 transition-colors group-hover:stroke-white" />
                  <span className="text-xs">
                    Upload a file or drag and drop (optional)
                  </span>
                </div>
              )}
              <input
                {...createJobForm.register("file")}
                {...getInputFileProps({
                  onChange: createJobForm.register("file").onChange,
                })}
              />
              <div>
                {file && (
                  <img
                    alt=""
                    className="absolute left-0 top-0 h-full w-full object-cover"
                    src={file.preview}
                  />
                )}
              </div>
              {file != null && (
                <button
                  className={cn(
                    "absolute -right-1 -top-1 cursor-pointer rounded-full bg-slate-700 hover:bg-slate-900",
                    file ? "block" : "hidden"
                  )}
                  onClick={(event) => {
                    event.stopPropagation();
                    createJobForm.setValue("file", undefined, { shouldValidate: true });
                  }}
                >
                  <XCircleIcon className="h-6 w-6" />
                </button>
              )}
            </div>
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
