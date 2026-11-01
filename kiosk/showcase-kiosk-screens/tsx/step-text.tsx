// Kiosk reference screen, phase Step, text fields: a name, and a
// reference number of digits. Each field shows its error beside it, and
// limits what can be typed to what the form accepts.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Column, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 32;

type AboutYouProps = {
  nameError: string;
  referenceError: string;
  onCancel: () => void;
  onNext: () => void;
};

export function AboutYouStep(props: AboutYouProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">About you</Text>
        <Row gap={GAP}>
          <Text>Your name</Text>
          <TextInput id="name" placeholder="First and last name" maxLength={32} />
        </Row>
        {props.nameError && <Text>{props.nameError}</Text>}
        <Row gap={GAP}>
          <Text>Reference (optional)</Text>
          <TextInput id="reference" placeholder="Digits only" inputMode="numeric" maxLength={10} />
        </Row>
        {props.referenceError && <Text>{props.referenceError}</Text>}
        <Spacer size={GAP_ACTIONS} />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
