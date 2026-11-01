// Pay at the pump, the idle warning before the privacy reset.
// Fictional brand; every text invented.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const GAP_ACTIONS = 16;
const EDGE = 24;

type IdleWarningProps = {
  secondsLeft: number;
  onStillHere: () => void;
  onCancel: () => void;
};

export function IdleWarningScreen(props: IdleWarningProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title" role="title">
          Are you still there?
        </Text>
        <Text>For the next customer, this screen clears itself.</Text>
        <Row gap={GAP}>
          <Text>Seconds left:</Text>
          <Text>{props.secondsLeft}</Text>
        </Row>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button variant="quiet" onPress={props.onCancel}>
            Clear it now
          </Button>
          <Spacer />
          <Button id="still-here" onPress={props.onStillHere}>
            Yes, I am here
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
