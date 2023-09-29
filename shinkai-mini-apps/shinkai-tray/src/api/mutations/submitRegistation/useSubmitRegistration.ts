import { useMutation } from "@tanstack/react-query";
import type { UseMutationOptions } from "@tanstack/react-query";
import { SetupDataArgs, submitRegistration } from ".";
import { APIUseRegistrationCodeSuccessResponse } from "../../../shinkai-message-ts/src/models";

type Data = {
  success: boolean;
  data?: APIUseRegistrationCodeSuccessResponse | undefined;
};
type Options = UseMutationOptions<Data, Error, SetupDataArgs>;

export const useSubmitRegistration = (options?: Options) => {
  return useMutation({
    mutationFn: submitRegistration,
    ...options,
  });
};
