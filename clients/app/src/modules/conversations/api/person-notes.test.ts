import { describe, expect, it } from "vitest";

import { answeredPerson } from "./person-notes";

const SAM = { name: "Sam Rivera", address: "sam@example.com" };
const MARTA = { name: "Marta Ferreira", address: " Marta@Clientes.example " };

describe("answeredPerson", () => {
  it("is the sender of mail received, the first recipient of mail sent", () => {
    expect(answeredPerson({ direction: "inbound", from: MARTA, to: [SAM] })).toEqual({
      address: "marta@clientes.example",
      name: "Marta Ferreira",
    });
    expect(
      answeredPerson({
        direction: "outbound",
        from: SAM,
        to: [{ address: "rui@obras.example" }, MARTA],
      }),
    ).toEqual({ address: "rui@obras.example", name: undefined });
    expect(answeredPerson({ direction: "outbound", from: SAM, to: [] })).toBeUndefined();
  });

  it("is nobody when the address is not plain, as the server reads it", () => {
    for (const address of [
      "marta",
      "@clientes.example",
      "a@b@c.example",
      'x"@y.example',
      "x\\y@z.example",
      "marta @clientes.example",
      "marta@clientes.example\u0007",
    ]) {
      expect(
        answeredPerson({ direction: "inbound", from: { address }, to: [SAM] }),
        address,
      ).toBeUndefined();
    }
  });
});
