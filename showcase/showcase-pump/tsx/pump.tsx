// Pay at the pump, step 1: which pump the driver is at, one of
// four. Fictional brand; every text invented.
import { Button, Column, RadioGroup, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 10;
const GAP_ACTIONS = 16;
const EDGE = 24;

type PumpProps = {
  pumpError: string;
  onCancel: () => void;
  onNext: () => void;
};

export function PumpStep(props: PumpProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading" role="title">
          Which pump are you at?
        </Text>
        <Text>The number is on the pump, above the nozzles.</Text>
        <RadioGroup id="pump" options={["Pump 1", "Pump 2", "Pump 3", "Pump 4"]} />
        {props.pumpError && <Text>{props.pumpError}</Text>}
        <Spacer />
        <Row gap={GAP_ACTIONS}>
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
