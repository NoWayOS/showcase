// Kiosk reference screen, phase Submitted: the service accepted the
// answers, which are already cleared; the screen clears itself, counting
// down the seconds left - a <Text> of its own, as on the idle
// warning.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 32;

type SubmittedProps = {
  secondsLeft: number;
  onDone: () => void;
};

export function SubmittedScreen({ secondsLeft, onDone }: SubmittedProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Thank you</Text>
        <Text>Your question is with the service desk.</Text>
        <Text>This screen clears itself.</Text>
        <Row gap={GAP}>
          <Text>Seconds left:</Text>
          <Text>{secondsLeft}</Text>
        </Row>
        <Spacer />
        <Button id="done" onPress={onDone}>
          Done
        </Button>
      </Column>
    </Screen>
  );
}
