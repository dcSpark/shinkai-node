import React, { useEffect, useState } from "react";
import { useDispatch, useSelector } from "react-redux";
import {
  IonContent,
  IonHeader,
  IonPage,
  IonTitle,
  IonToolbar,
  IonButton,
  IonInput,
  IonLabel,
  IonItem,
  IonToast,
  InputChangeEventDetail,
} from "@ionic/react";
import { submitRegistrationCode } from "../api";
import { BrowserQRCodeReader } from "@zxing/browser";
import { Camera, CameraResultType, CameraSource } from "@capacitor/camera";
import { useHistory } from "react-router-dom";
import { toast } from "react-toastify";
import type { AppDispatch, RootState } from "../store";
import { QrScanner } from "@yudiel/react-qr-scanner";
import { BarcodeScanner } from "@capacitor-community/barcode-scanner";
import { isPlatform } from "@ionic/react";
import {
  generateEncryptionKeys,
  generateSignatureKeys,
} from "../utils/wasm_helpers";
import { QRSetupData } from "../models/QRSetupData";
import { SetupDetailsState } from "../store/reducers";
import { InputCustomEvent } from "@ionic/core/dist/types/components/input/input-interface";
import { cn } from "../theme/lib/utils";
import Button from "../components/ui/Button";

export type MergedSetupType = SetupDetailsState & QRSetupData;

const Connect: React.FC = () => {
  const [setupData, setSetupData] = useState<MergedSetupType>({
    registration_code: "",
    profile: "main",
    registration_name: "main_device",
    identity_type: "device",
    permission_type: "admin",
    node_address: "",
    shinkai_identity: "",
    node_encryption_pk: "",
    node_signature_pk: "",
    myEncryptionPk: "",
    myEncryptionSk: "",
    myIdentityPk: "",
    myIdentitySk: "",
  });
  const [error, setError] = useState<string | null>(null);
  const dispatch = useDispatch<AppDispatch>();
  const history = useHistory();
  const errorFromState = useSelector((state: RootState) => state.error);

  // Generate keys when the component mounts
  useEffect(() => {
    // Assuming the seed is a random 32 bytes array.
    const seed = crypto.getRandomValues(new Uint8Array(32));
    generateEncryptionKeys(seed).then(
      ({ my_encryption_sk_string, my_encryption_pk_string }) =>
        setSetupData((prevState) => ({
          ...prevState,
          myEncryptionPk: my_encryption_pk_string,
          myEncryptionSk: my_encryption_sk_string,
        })),
    );
    generateSignatureKeys().then(
      ({ my_identity_pk_string, my_identity_sk_string }) =>
        setSetupData((prevState) => ({
          ...prevState,
          myIdentityPk: my_identity_pk_string,
          myIdentitySk: my_identity_sk_string,
        })),
    );
  }, []);

  const updateSetupData = (data: Partial<MergedSetupType>) => {
    setSetupData((prevState) => ({ ...prevState, ...data }));
  };

  const handleScan = async (data: any) => {
    if (data) {
      const result = JSON.parse(data);
      console.log("Prev. QR Code Data:", setupData);
      updateSetupData(result);
      console.log("New QR Code Data:", setupData);
    }
  };

  const handleImageUpload = async () => {
    try {
      const image = await Camera.getPhoto({
        quality: 90,
        allowEditing: true,
        resultType: CameraResultType.DataUrl,
        source: isPlatform("desktop")
          ? CameraSource.Photos
          : CameraSource.Prompt,
      });
      const codeReader = new BrowserQRCodeReader();
      const resultImage = await codeReader.decodeFromImageUrl(image.dataUrl);
      const json_string = resultImage.getText();
      const parsedData: QRSetupData = JSON.parse(json_string);
      updateSetupData(parsedData);
    } catch (error) {
      console.error("Error uploading image:", error);
    }
  };

  const handleError = (err: any) => {
    console.error(err);
  };

  const handleQRScan = async () => {
    if (isPlatform("capacitor")) {
      const result = await BarcodeScanner.startScan();
      if (result.hasContent) {
        handleScan(result.content);
      }
    }
  };

  const finishSetup = async () => {
    const success = await dispatch(submitRegistrationCode(setupData));

    if (success) {
      localStorage.setItem("setupComplete", "true");
      history.push("/home");
    } else {
      console.log("Error from state:", errorFromState);
      toast.error(errorFromState);
    }
  };

  return (
    <IonPage>
      <IonHeader className="shadow">
        <IonToolbar>
          <IonTitle className="container text-accent text-center">
            Connect
          </IonTitle>
        </IonToolbar>
      </IonHeader>
      <IonContent fullscreen>
        {error && <IonToast color="danger" message={error} duration={2000} />}
        <div className="grid md:grid-cols-[45%_55%]">
          <div className="hidden md:block h-full">
            <img
              src="https://shinkai.com/assets/bg-hero.webp"
              className="fixed object-cover object-bottom w-full h-full blur-sm"
              alt=""
            />
          </div>
          <div className="bg-white relative w-full">
            <div className="mt-6 mb-6 mx-auto w-full md:w-[500px] space-y-5 p-6 rounded-3xl bg-white/30 shadow-[0px 29.04092788696289px 36.3011589050293px 0px rgba(0, 0, 0, 0.05)] backdrop-blur-[64px]">
              <Input
                value={setupData.registration_code}
                onChange={(e) =>
                  updateSetupData({ registration_code: e.detail.value! })
                }
                label="Registration Code"
              />
              <Input
                value={setupData.registration_name}
                onChange={(e) =>
                  updateSetupData({ registration_name: e.detail.value! })
                }
                label="Registration Name (Your choice)"
              />
              <Input
                value={setupData.node_address}
                onChange={(e) =>
                  updateSetupData({ node_address: e.detail.value! })
                }
                label="Node Address (IP:PORT)"
              />
              <Input
                value={setupData.shinkai_identity}
                onChange={(e) =>
                  updateSetupData({ shinkai_identity: e.detail.value! })
                }
                label="Shinkai Identity (@@IDENTITY.shinkai)"
              />
              <Input
                value={setupData.node_encryption_pk}
                onChange={(e) =>
                  updateSetupData({ node_encryption_pk: e.detail.value! })
                }
                label="Node Encryption Public Key"
              />
              <Input
                value={setupData.node_signature_pk}
                onChange={(e) =>
                  updateSetupData({ node_signature_pk: e.detail.value! })
                }
                label="Node Signature Public Key"
              />
              <Input
                value={setupData.myEncryptionPk}
                onChange={(e) =>
                  updateSetupData({ myEncryptionPk: e.detail.value! })
                }
                label="My Encryption Public Key"
              />
              <Input
                value={setupData.myIdentityPk}
                onChange={(e) =>
                  updateSetupData({ myIdentityPk: e.detail.value! })
                }
                label="My Signature Public Key"
              />

              <Button onClick={handleImageUpload}>Upload Image</Button>
              {isPlatform("capacitor") ? (
                <Button onClick={handleQRScan}>Scan QR Code</Button>
              ) : (
                <QrScanner
                  scanDelay={300}
                  onError={handleError}
                  onDecode={handleScan}
                  containerStyle={{ width: "100%" }}
                />
              )}
              <Button onClick={finishSetup}>Finish Setup</Button>
            </div>
          </div>
        </div>
      </IonContent>
    </IonPage>
  );
};

export default Connect;

function Input({
  onChange,
  value,
  label,
  className,
}: {
  onChange: (event: InputCustomEvent<InputChangeEventDetail>) => void;
  value: string;
  label: string;
  className?: string;
}) {
  return (
    <IonItem
      className={cn("[--ion-item-border-color:--ion-color-primary]", className)}
      shape="round"
    >
      <IonInput
        className="flex gap-10"
        value={value}
        onIonChange={onChange}
        labelPlacement="stacked"
        shape="round"
        label={label}
        aria-label={label}
      />
    </IonItem>
  );
}
