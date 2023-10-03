import React from "react";

export type AuthContextType = {
  setupData: SetupData | null;
  setSetupData: React.Dispatch<React.SetStateAction<SetupData | null>>;
};

export type SetupData = {
  profile: string;
  permission_type: string;
  registration_name: string;
  node_address: string;
  shinkai_identity: string;
  node_encryption_pk: string;
  node_signature_pk: string;
  profile_encryption_sk: string;
  profile_encryption_pk: string;
  profile_identity_sk: string;
  profile_identity_pk: string;
  my_device_encryption_sk: string;
  my_device_encryption_pk: string;
  my_device_identity_sk: string;
  my_device_identity_pk: string;
};

function useLocalStorage<T>(
  key: string,
  defaultValue: T
): [T, React.Dispatch<React.SetStateAction<T>>] {
  const [value, setValue] = React.useState(() => {
    const saved = localStorage.getItem(key);
    console.log("saved", saved);
    if (!saved) {
      return defaultValue;
    }

    return saved === undefined ? defaultValue : JSON.parse(saved);
  });

  React.useEffect(() => {
    localStorage.setItem(key, JSON.stringify(value));
  }, [key, value]);

  return [value, setValue];
}

const AuthContext = React.createContext<AuthContextType>({} as AuthContextType);

export const AuthProvider = ({ children }: { children: React.ReactNode }) => {
  const [setupData, setSetupData] = useLocalStorage<SetupData | null>("setup", null);

  return (
    <AuthContext.Provider value={{ setupData, setSetupData }}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = React.useContext(AuthContext);
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider");
  }
  return context;
};
