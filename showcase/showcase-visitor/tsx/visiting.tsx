// Visitor sign-in, step 2: who the visitor has come to see, as free
// text: the kiosk holds no list of staff, and reception checks the name.
// Fictional brand; every text invented.
import { Button, Column, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 24;

type VisitingProps = {
  hostError: string;
  onBack: () => void;
  onCancel: () => void;
  onNext: () => void;
};

export function VisitingStep(props: VisitingProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Who are you visiting?</Text>
        <Row gap={GAP}>
          <Text>Their name</Text>
          <TextInput id="host" placeholder="The person you are here to see" maxLength={32} />
        </Row>
        {props.hostError && <Text>{props.hostError}</Text>}
        <Text>Reception checks the name: this kiosk has no list of staff.</Text>
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
