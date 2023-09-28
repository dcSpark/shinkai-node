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
import { Textarea } from "../components/ui/textarea";
import { useAuth } from "../store/auth-context";
import { useNavigate } from "react-router-dom";
import { Input } from "../components/ui/input";
import { useCreateChat } from "../api/mutations/createChat/useCreateChat";

const createChatSchema = z.object({
  receiver: z.string(),
  message: z.string().min(1, "Message cannot be empty"),
});

const CreateChatPage = () => {
  const { setupData } = useAuth();
  const navigate = useNavigate();

  const { isLoading, mutateAsync: createChat } = useCreateChat({
    onSuccess: (data) => {
      console.log(data, "chat");
      // TODO: job_inbox, false is hardcoded
      navigate(`/inboxes/${data.inboxId}`);
    },
  });

  const createChatForm = useForm<z.infer<typeof createChatSchema>>({
    resolver: zodResolver(createChatSchema),
  });

  const onSubmit = async (data: z.infer<typeof createChatSchema>) => {
    if (!setupData) return;
    const [receiver, ...rest] = data.receiver.split("/");

    createChat({
      sender: setupData.shinkai_identity,
      senderSubidentity: `${setupData.profile}/device/${setupData.registration_name}`,
      receiver,
      receiverSubidentity: rest.join("/"),
      message: data.message,
      my_device_encryption_sk: setupData.my_device_encryption_sk,
      my_device_identity_sk: setupData.my_device_identity_sk,
      node_encryption_pk: setupData.node_encryption_pk,
    });
  };
  return (
    <SimpleLayout title="Create Chat">
      <Form {...createChatForm}>
        <form className="space-y-10" onSubmit={createChatForm.handleSubmit(onSubmit)}>
          <div className="space-y-6">
            <FormField
              control={createChatForm.control}
              name="receiver"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Shinkai Identity</FormLabel>
                  <FormControl>
                    <Input
                      placeholder="Eg: @@name.shinkai or @@name.shinkai/profile"
                      {...field}
                    />
                  </FormControl>
                </FormItem>
              )}
            />
            <FormField
              control={createChatForm.control}
              name="message"
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Message</FormLabel>
                  <FormControl>
                    <Textarea
                      placeholder="Enter your message"
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
            Create Chat
          </Button>
        </form>
      </Form>
    </SimpleLayout>
  );
};
export default CreateChatPage;
