import { Button } from "../components/ui/button.tsx";
import {
  Form,
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "../components/ui/form.tsx";
import SimpleLayout from "./layout/simple-layout.tsx";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../components/ui/select.tsx";
import { Input } from "../components/ui/input.tsx";
import { z } from "zod";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

enum IdentityType {
  Profile = "profile",
  Device = "device",
}

enum PermissionType {
  Admin = "admin",
  Standard = "standard",
  None = "none",
}

const generateCodeSchema = z.object({
  identityType: z.nativeEnum(IdentityType),
  profile: z.string().min(1),
  permissionType: z.nativeEnum(PermissionType),
});

const identityTypeOptions = [IdentityType.Profile, IdentityType.Device];
const permissionOptions = [
  PermissionType.Admin,
  PermissionType.Standard,
  PermissionType.None,
];

const GenerateCodePage = () => {
  const form = useForm<z.infer<typeof generateCodeSchema>>({
    resolver: zodResolver(generateCodeSchema),
    defaultValues: {
      profile: "",
      permissionType: PermissionType.Admin,
      identityType: IdentityType.Device,
    },
  });

  const { identityType } = form.watch();
  const onSubmit = async (data: z.infer<typeof generateCodeSchema>) => {
    console.log("qwqweqwe", data);
  };
  return (
    <SimpleLayout title="Generate Registration Code">
      <Form {...form}>
        <form
          className="flex flex-col justify-between space-y-3"
          onSubmit={form.handleSubmit(onSubmit)}
        >
          <div className="flex grow flex-col space-y-2">
            <FormField
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Select Identity Type</FormLabel>
                  <Select onValueChange={field.onChange} value={field.value}>
                    <FormControl>
                      <SelectTrigger>
                        <SelectValue placeholder="Select your AI Agent" />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                      {identityTypeOptions.map((option) => (
                        <SelectItem key={option} value={option}>
                          <span>{option} </span>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FormItem>
              )}
              control={form.control}
              name="identityType"
            />

            {identityType === IdentityType.Device && (
              <FormField
                control={form.control}
                name="profile"
                render={({ field }) => (
                  <FormItem>
                    <FormLabel>Profile</FormLabel>
                    <FormControl>
                      <Input {...field} />
                    </FormControl>
                    <FormMessage />
                  </FormItem>
                )}
              />
            )}

            <FormField
              render={({ field }) => (
                <FormItem>
                  <FormLabel>Select Permission Type</FormLabel>
                  <Select onValueChange={field.onChange} value={field.value}>
                    <FormControl>
                      <SelectTrigger>
                        <SelectValue placeholder="Select your AI Agent" />
                      </SelectTrigger>
                    </FormControl>
                    <SelectContent>
                      {permissionOptions.map((option) => (
                        <SelectItem key={option} value={option}>
                          <span>{option} </span>
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </FormItem>
              )}
              control={form.control}
              name="permissionType"
            />
          </div>
          <Button className="w-full" type="submit">
            Generate Code
          </Button>
        </form>
      </Form>
    </SimpleLayout>
  );
};

export default GenerateCodePage;
