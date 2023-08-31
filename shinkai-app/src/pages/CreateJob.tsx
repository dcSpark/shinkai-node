// pages/CreateJob.tsx
import {
  IonPage,
  IonHeader,
  IonToolbar,
  IonTitle,
  IonContent,
  IonButton,
  IonInput,
  IonLabel,
  IonItem,
  IonGrid,
  IonRow,
  IonCol,
  IonButtons,
  IonBackButton,
} from "@ionic/react";
import { useState } from "react";
import { IonContentCustom, IonHeaderCustom } from "../components/ui/Layout";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { createJob, sendMessageToJob } from "../api";
import { useSetup } from "../hooks/usetSetup";
import { useDispatch, useSelector } from "react-redux";
import { RootState } from "../store";

const CreateJob: React.FC = () => {
  useSetup();
  const dispatch = useDispatch();
  const setupDetailsState = useSelector(
    (state: RootState) => state.setupDetailsState
  );
  const [jobContent, setJobContent] = useState("");

  const handleCreateJob = () => {
    // try {
    //   // Perform your API request here
    console.log("Creating job with content:", jobContent);
    //   // We should show a list of all the available agents

    //   // Split shinkaiIdentity into sender and the rest
    // let [receiver, ...rest] = shinkaiIdentity.split("/");

    // // Join the rest back together to form sender_subidentity
    // let receiver_subidentity = rest.join("/");

    //   const { shinkai_identity, profile, registration_name } = setupDetailsState;

    //   // Define your parameters for createJob
    //   const scope = {};
    //   let sender = shinkai_identity;
    // let sender_subidentity = `${profile}/device/${registration_name}`;

    //   const receiver = ...;
    //   const receiver_subidentity = ...;
    //   const setupDetailsState = ...;

    //   // Call createJob
    //   const jobId = await dispatch(createJob(scope, sender, receiver, receiver_subidentity, setupDetailsState));

    //   await dispatch(sendMessageToJob(jobId, jobContent, sender, receiver, receiver_subidentity, setupDetailsState));

    // } catch (error) {
    //   console.error("Error in handleCreateJob:", error);
    // }
  };

  return (
    <IonPage>
      <IonHeaderCustom>
        <IonButtons slot="start">
          <IonBackButton defaultHref="/home" />
        </IonButtons>
        <IonTitle>Create Job</IonTitle>
      </IonHeaderCustom>
      <IonContentCustom>
        <IonGrid
          className={
            "md:rounded-[1.25rem] bg-white dark:bg-slate-800 p-4 md:p-10 space-y-2 md:space-y-4"
          }
        >
          <IonRow>
            <IonCol>
              <h2 className={"text-lg mb-3 md:mb-8 text-center"}>
                New Job Details
              </h2>
              <Input
                value={jobContent}
                label="Tell me the job to do"
                aria-label="Tell me the job to do"
                onChange={(e) => setJobContent(e.detail.value!)}
              />

              <div style={{ marginTop: "20px" }}>
                <Button onClick={handleCreateJob}>Create Job</Button>
              </div>
            </IonCol>
          </IonRow>
        </IonGrid>
      </IonContentCustom>
    </IonPage>
  );
};

export default CreateJob;
