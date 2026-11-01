// Pay at the pump, step 3: how much, one of four preset amounts:
// the pump stops there. Fictional brand; every text invented.
import { Button, Column, RadioGroup, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 10;
const GAP_ACTIONS = 16;
const EDGE = 24;

type AmountProps = {
  amountError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function AmountStep(props: AmountProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading" role="title">
          How much fuel?
        </Text>
        <Text>The pump stops at this amount.</Text>
        <RadioGroup id="amount" options={["£10", "£20", "£40", "£60"]} />
        {props.amountError && <Text>{props.amountError}</Text>}
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button variant="quiet" onPress={props.onCancel}>
            Cancel
          </Button>
          <Spacer />
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
