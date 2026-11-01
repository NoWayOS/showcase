// Kiosk reference screen, phase Submitting: the answers are on
// their way to the service, and nothing can be changed or cancelled.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Column, Screen, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 32;

export function SubmittingScreen() {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Sending your answers</Text>
        <Text>Please wait. This takes a few seconds.</Text>
      </Column>
    </Screen>
  );
}
