// pages/AddAgent.tsx
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
  InputChangeEventDetail,
} from "@ionic/react";
import { useEffect, useState } from "react";
import { IonContentCustom, IonHeaderCustom } from "../components/ui/Layout";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { useDispatch, useSelector } from "react-redux";
import { RootState } from "../store";
import { SerializedAgent, AgentAPIModel } from "../models/SchemaTypes";

const AddAgent: React.FC = () => {
  const dispatch = useDispatch();
  const [agent, setAgent] = useState<Partial<SerializedAgent>>({});

  const handleInputChange = (event: CustomEvent<InputChangeEventDetail>) => {
    const name = (event.target as HTMLInputElement).name;
    const value = event.detail.value;
    if (name) {
      setAgent((prevState) => ({ ...prevState, [name]: value }));
    }
  };

  const handleSubmit = () => {
    // Here you would typically dispatch an action to add the agent
    // For now, we'll just log the agent data
    console.log(agent);
  };

  return (
    <IonPage>
      <IonHeaderCustom>
        <IonButtons slot="start">
          <IonBackButton defaultHref="/home" />
        </IonButtons>
        <IonTitle>Add Agent</IonTitle>
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
                New Agent Details
              </h2>

              <IonItem>
                <IonLabel>Full Identity Name</IonLabel>
                <IonInput
                  id="full_identity_name"
                  value={agent.full_identity_name}
                  onIonChange={handleInputChange}
                />
              </IonItem>

              <IonItem>
                <IonLabel>Perform Locally</IonLabel>
                <IonInput
                  type="checkbox"
                  name="perform_locally"
                  checked={agent.perform_locally}
                  onIonChange={(e) =>
                    setAgent((prevState) => ({
                      ...prevState,
                      perform_locally: e.detail.checked,
                    }))
                  }
                />
              </IonItem>

              {/* Add similar IonItem components for other properties of the agent */}

              <div style={{ marginTop: "20px" }}>
                <Button onClick={handleSubmit}>Add Agent</Button>
              </div>
            </IonCol>
          </IonRow>
        </IonGrid>
      </IonContentCustom>
    </IonPage>
  );
};

export default AddAgent;
