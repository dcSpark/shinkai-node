import { useEffect } from "react";
import {
  generateEncryptionKeys,
  generateSignatureKeys,
} from "@shinkai_network/shinkai-message-ts/utils";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "../components/ui/form";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useNavigate } from "react-router-dom";
import { HOME_PATH } from "../routes/name";
import { useAuth } from "../store/auth-context";
import { useSubmitRegistration } from "../api/mutations/submitRegistation/useSubmitRegistration";
import ErrorMessage from "../components/ui/error-message";
import { queryClient } from "../api/constants";

const formSchema = z.object({
  registration_code: z.string(),
  profile: z.string(),
  registration_name: z.string(),
  identity_type: z.string(),
  permission_type: z.string(),
  node_address: z.string().url({
    message: "Node Address must be a valid URL",
  }),
  shinkai_identity: z.string(),
  node_encryption_pk: z.string(),
  node_signature_pk: z.string(),
  profile_encryption_sk: z.string(),
  profile_encryption_pk: z.string(),
  profile_identity_sk: z.string(),
  profile_identity_pk: z.string(),
  my_device_encryption_sk: z.string(),
  my_device_encryption_pk: z.string(),
  my_device_identity_sk: z.string(),
  my_device_identity_pk: z.string(),
});

const OnboardingPage = () => {
  const navigate = useNavigate();
  const { setSetupData } = useAuth();

  const setupDataForm = useForm<z.infer<typeof formSchema>>({
    resolver: zodResolver(formSchema),
    defaultValues: {
      node_address: "http://localhost:9550",
      registration_code: "",
      profile: "main",
      registration_name: "main_device",
      identity_type: "device",
      permission_type: "admin",
      shinkai_identity: "@@node1.shinkai", // this should actually be read from ENV
      node_encryption_pk: "",
      node_signature_pk: "",
      profile_encryption_sk: "",
      profile_encryption_pk: "",
      profile_identity_sk: "",
      profile_identity_pk: "",
      my_device_encryption_sk: "",
      my_device_encryption_pk: "",
      my_device_identity_sk: "",
      my_device_identity_pk: "",
    },
  });

  const {
    isLoading,
    isError,
    error,
    mutateAsync: submitRegistration,
  } = useSubmitRegistration({
    onSuccess: (response) => {
      if (response.success) {
        const responseData = response.data;
        const updatedSetupData = {
          ...setupDataForm.getValues(),
          node_encryption_pk: responseData?.encryption_public_key ?? "",
          node_signature_pk: responseData?.identity_public_key ?? "",
        };
        setSetupData(updatedSetupData);
        navigate(HOME_PATH);
      } else {
        throw new Error("Failed to submit registration");
      }
    },
  });

  useEffect(() => {
    // clean up
    setSetupData(null);
    queryClient.clear();

    fetch("http://127.0.0.1:9550/v1/shinkai_health")
      .then((response) => response.json())
      .then((data) => {
        if (data.status === "ok") {
          setupDataForm.setValue("node_address", "http://127.0.0.1:9550");
        }
      })
      .catch((error) => console.error("Error:", error));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Generate keys when the component mounts
  useEffect(() => {
    // Assuming the seed is a random 32 bytes array.
    // Device Keys
    let seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(
      ({ my_encryption_sk_string, my_encryption_pk_string }) => {
        setupDataForm.setValue("my_device_encryption_pk", my_encryption_pk_string);
        setupDataForm.setValue("my_device_encryption_sk", my_encryption_sk_string);
      }
    );
    generateSignatureKeys().then(({ my_identity_pk_string, my_identity_sk_string }) => {
      setupDataForm.setValue("my_device_identity_pk", my_identity_pk_string);
      setupDataForm.setValue("my_device_identity_sk", my_identity_sk_string);
    });

    // Profile Keys
    seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(
      ({ my_encryption_sk_string, my_encryption_pk_string }) => {
        setupDataForm.setValue("profile_encryption_pk", my_encryption_pk_string);
        setupDataForm.setValue("profile_encryption_sk", my_encryption_sk_string);
      }
    );
    generateSignatureKeys().then(({ my_identity_pk_string, my_identity_sk_string }) => {
      setupDataForm.setValue("profile_identity_pk", my_identity_pk_string);
      setupDataForm.setValue("profile_identity_sk", my_identity_sk_string);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function onSubmit(currentValues: z.infer<typeof formSchema>) {
    submitRegistration(currentValues);
  }

  return (
    <div className="p-10 max-w-lg mx-auto">
      <h1 className="text-center text-3xl font-semibold mb-4">Register</h1>
      <Form {...setupDataForm}>
        <form onSubmit={setupDataForm.handleSubmit(onSubmit)} className="space-y-8">
          <FormField
            control={setupDataForm.control}
            name="node_address"
            render={({ field }) => (
              <FormItem>
                <FormLabel>Node Address</FormLabel>
                <FormControl>
                  <Input placeholder="Eg: http://localhost:9550" {...field} />
                </FormControl>
                <FormMessage />
              </FormItem>
            )}
          />
          {isError && <ErrorMessage message={error.message} />}
          <Button
            className="w-full"
            variant="default"
            type="submit"
            isLoading={isLoading}
            disabled={isLoading}
          >
            Submit
          </Button>
        </form>
      </Form>
    </div>
  );
};

export default OnboardingPage;
