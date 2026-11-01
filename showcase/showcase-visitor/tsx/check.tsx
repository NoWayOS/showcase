// Visitor sign-in, check your details: every answer, from the UI's
// own copies, then sign in - or, for a demonstration, sign in with the
// building service down. Fictional brand; every text invented.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 6;
const GAP_ACTIONS = 12;
const EDGE = 24;

type CheckProps = {
  name: string;
  organisation: string;
  host: string;
  purpose: string;
  onBack: () => void;
  onCancel: () => void;
  onServiceDown: () => void;
  onSignIn: () => void;
};

export function CheckScreen(props: CheckProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Check your details</Text>
        <Row gap={GAP}>
          <Text>Name:</Text>
          <Text>{props.name}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Organisation:</Text>
          <Text>{props.organisation}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Visiting:</Text>
          <Text>{props.host}</Text>
        </Row>
        <Row gap={GAP}>
          <Text>Purpose:</Text>
          <Text>{props.purpose}</Text>
        </Row>
        <Text>Site rules: agreed</Text>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Change</Button>
          <Button onPress={props.onCancel}>Cancel</Button>
          <Spacer />
          <Button id="service-down" onPress={props.onServiceDown}>
            Demo: service down
          </Button>
          <Button id="sign-in" onPress={props.onSignIn}>
            Sign in
          </Button>
        </Row>
      </Column>
    </Screen>
  );
}
