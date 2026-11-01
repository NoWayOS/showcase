// Visitor sign-in, the sign-in did not reach reception: say so
// plainly and send the visitor to the desk; try again when it may help.
// Fictional brand; every text invented.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 10;
const GAP_ACTIONS = 16;
const EDGE = 24;

type NotSignedInProps = {
  canRetry: boolean;
  onRetry: () => void;
  onBack: () => void;
  onCancel: () => void;
};

export function NotSignedInScreen(props: NotSignedInProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title">Reception does not know you are here yet</Text>
        <Text>Please go to the reception desk and tell them who you are visiting.</Text>
        <Text>Demo: the building service is simulated as down</Text>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button onPress={props.onBack}>Check details</Button>
          <Button onPress={props.onCancel}>Finish</Button>
          <Spacer />
          {props.canRetry && (
            <Button id="retry" onPress={props.onRetry}>
              Try again
            </Button>
          )}
        </Row>
      </Column>
    </Screen>
  );
}
