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
  IonSelect,
  IonSelectOption,
  IonTextarea,
} from "@ionic/react";
import { useEffect, useState } from "react";
import { IonContentCustom, IonHeaderCustom } from "../components/ui/Layout";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { createJob, getProfileAgents, sendMessageToJob } from "../api";
import { useSetup } from "../hooks/usetSetup";
import { useDispatch, useSelector } from "react-redux";
import { RootState } from "../store";
import { SerializedAgent } from "../models/SchemaTypes";

const CreateJob: React.FC = () => {
  useSetup();
  const dispatch = useDispatch();
  const setupDetailsState = useSelector(
    (state: RootState) => state.setupDetailsState
  );
  const [jobContent, setJobContent] = useState("");
  const [selectedAgent, setSelectedAgent] = useState("");
  const [agents, setAgents] = useState<SerializedAgent[]>([]);

  useEffect(() => {
    const fetchAgents = async () => {
      const { shinkai_identity, profile, registration_name } = setupDetailsState;
      let node_name = shinkai_identity;
      let sender_subidentity = `${profile}/device/${registration_name}`;
  
      const agentsData = await getProfileAgents(node_name, sender_subidentity, node_name, setupDetailsState)(dispatch);
      if (Array.isArray(agentsData)) {
        dispatch(setAgents(agentsData));
      } else {
        console.error("Received data is not an array of agents");
      }
    };
  
    fetchAgents();
  }, [dispatch, setupDetailsState]);

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

              <IonItem>
                <IonLabel>Select Agent</IonLabel>
                <IonSelect
                  value={selectedAgent}
                  placeholder="Select One"
                  onIonChange={(e) => setSelectedAgent(e.detail.value)}
                >
                  {agents.map((agent, index) => (
                    <IonSelectOption key={index} value={agent}>
                      {agent.id}
                    </IonSelectOption>
                  ))}
                </IonSelect>
              </IonItem>

              <IonItem>
                <IonLabel position="floating">Tell me the job to do</IonLabel>
                <IonTextarea
                  value={jobContent}
                  onIonChange={(e) => setJobContent(e.detail.value!)}
                />
              </IonItem>

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
