import { EncryptionMethod, ShinkaiMessageBuilderWrapper, ShinkaiMessageWrapper as ShinkaiMessageWrapperWASM } from '../../pkg/shinkai_message_wasm.js';
import { Body, InternalMetadata, ExternalMetadata } from '../../models/ShinkaiMessage';
import { mapEncryptionMethod } from '../../utils/wasm_types_conversion.js';
// import * as wasm from './pkg/shinkai_message_wasm.js';



export class ShinkaiMessageWrapper {
  private wasmWrapper: ShinkaiMessageWrapperWASM;

  constructor(body: Body, external_metadata: ExternalMetadata, encryption: EncryptionMethod) {
    this.wasmWrapper = new ShinkaiMessageWrapperWASM(body, external_metadata, encryption);
  }

  static fromJsValue(j: any): ShinkaiMessageWrapper {
    const wasmWrapper = ShinkaiMessageWrapperWASM.fromJsValue(j);
    return new ShinkaiMessageWrapper(wasmWrapper.body, wasmWrapper.external_metadata, mapEncryptionMethod(wasmWrapper.encryption));
  }

  to_jsvalue(): any {
    return this.wasmWrapper.to_jsvalue();
  }

  to_json_str(): string {
    return this.wasmWrapper.to_json_str();
  }

  static from_json_str(s: string): ShinkaiMessageWrapper {
    const wasmWrapper = ShinkaiMessageWrapperWASM.from_json_str(s);
    return new ShinkaiMessageWrapper(wasmWrapper.body, wasmWrapper.external_metadata, mapEncryptionMethod(wasmWrapper.encryption));
  }

  get body(): Body {
    return this.wasmWrapper.body;
  }

  get encryption(): string {
    return this.wasmWrapper.encryption;
  }

  get external_metadata(): ExternalMetadata {
    return this.wasmWrapper.external_metadata;
  }
}
