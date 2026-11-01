// Kiosk reference screen, phase Step, checkboxes: one the person
// must tick, with its error beside it, and one they may.
// Synthetic and sector-neutral; not taken from any product or customer.
import { Button, Checkbox, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 32;

type ConfirmProps = {
  agreeError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function ConfirmStep(props: ConfirmProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Confirm</Text>
        <Checkbox id="agree" label="Send my answers to the service desk" />
        {props.agreeError && <Text>{props.agreeError}</Text>}
        <Checkbox id="receipt" label="Print a receipt" />
        <Spacer size={GAP_ACTIONS} />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Back</Button>
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
