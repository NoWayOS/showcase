// Kiosk reference screen, the idle warning: nobody has touched the
// kiosk for a while, and the form clears itself unless someone answers. The
// countdown is a <Text> of its own, as Screen Compiler's subset takes a prop's
// text alone in its element.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const GAP_ACTIONS = 16;
const EDGE = 32;

type IdleWarningProps = {
  secondsLeft: number;
  onStillHere: () => void;
  onCancel: () => void;
};

export function IdleWarningScreen(props: IdleWarningProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Are you still there?</Text>
        <Text>For privacy, this form clears itself.</Text>
        <Row gap={GAP}>
          <Text>Seconds left:</Text>
          <Text>{props.secondsLeft}</Text>
        </Row>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onCancel}>Clear it now</Button>
          <Spacer />
          <Button id="still-here" onPress={props.onStillHere}>
            Yes, I am here
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
