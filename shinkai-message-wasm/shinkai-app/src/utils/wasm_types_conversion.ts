export function mapEncryptionMethod(encryption: String): number {
    switch (encryption) {
      case "DiffieHellmanChaChaPoly1305":
        return 0;
      case "None":
        return 1;
      default:
        throw new Error("Unknown encryption method");
    }
  }
  