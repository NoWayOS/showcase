// Pay at the pump, not accepted: the card reader declined the
// payment, so no pump was started. Try again while tries are left, change
// the choices, or finish. Fictional brand; every text invented.
import { Button, Column, Notice, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const GAP_ACTIONS = 16;
const EDGE = 24;
const TIGHT = 12;

type NotAcceptedProps = {
  canRetry: boolean;
  onRetry: () => void;
  onBack: () => void;
  onCancel: () => void;
};

export function NotAcceptedScreen(props: NotAcceptedProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="title" role="title">
          Payment not accepted
        </Text>
        <Notice tone="warning" padding={TIGHT}>
          <Text weight="bold">The card reader did not approve this payment. No pump was started.</Text>
        </Notice>
        <Text>Try again, or use another card. The card reader says why.</Text>
        <Text role="caption">Demo: the card was declined because you asked</Text>
        <Spacer />
        <Row gap={GAP_ACTIONS}>
          <Button variant="secondary" onPress={props.onBack}>
            Change
          </Button>
          <Button variant="quiet" onPress={props.onCancel}>
            Finish
          </Button>
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
