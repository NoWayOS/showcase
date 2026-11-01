// Visitor sign-in, step 3: why the visitor has come, one of four.
// Fictional brand; every text invented.
import { Button, Column, RadioGroup, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 24;

type PurposeProps = {
  purposeError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function PurposeStep(props: PurposeProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Why are you visiting?</Text>
        <RadioGroup id="purpose" options={["A meeting", "An interview", "A delivery", "Work on site"]} />
        {props.purposeError && <Text>{props.purposeError}</Text>}
        <Spacer />
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
