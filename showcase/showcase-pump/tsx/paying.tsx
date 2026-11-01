// Pay at the pump, paying: the driver pays on the card reader
// beside the screen, which learns only whether the payment was approved,
// and for how much. Fictional brand; every text invented.
import { Column, Notice, Screen, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;
const TIGHT = 12;

export function PayingScreen() {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title" role="title">
          Follow the instructions on the card reader
        </Text>
        <Text>The card reader is beside this screen. This takes a moment.</Text>
        <Notice tone="success" padding={TIGHT}>
          <Text weight="bold">This screen never sees your card, and cannot start a pump.</Text>
        </Notice>
        <Text role="caption">Demo: the card reader and the bank are simulated</Text>
      </Column>
    </Screen>
  );
}
