// Pay at the pump, step 2: which fuel, one of three, each with its
// price a litre. Fictional brand and prices; every text invented.
import { Button, Column, RadioGroup, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 10;
const GAP_ACTIONS = 16;
const EDGE = 24;

type FuelProps = {
  fuelError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function FuelStep(props: FuelProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading" role="title">
          Which fuel?
        </Text>
        <RadioGroup
          id="fuel"
          options={["Unleaded E10 - 142.9p a litre", "Super unleaded E5 - 156.9p a litre", "Diesel B7 - 149.9p a litre"]}
        />
        {props.fuelError && <Text>{props.fuelError}</Text>}
        <Text role="caption">Demo prices</Text>
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
