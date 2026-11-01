// Visitor sign-in, step 1: the visitor's name, and their
// organisation if they have one. Each field shows its error beside it.
// Fictional brand; every text invented.
import { Button, Column, Row, Screen, Spacer, Text, TextInput } from "nowayos:kit";

const GAP = 8;
const GAP_ACTIONS = 16;
const EDGE = 24;

type AboutYouProps = {
  nameError: string;
  organisationError: string;
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
          <Text>Organisation (optional)</Text>
          <TextInput id="organisation" placeholder="Where you are from" maxLength={32} />
        </Row>
        {props.organisationError && <Text>{props.organisationError}</Text>}
        <Spacer />
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
