// The Screen Compiler gallery, 4 of 11: the three variants, a button's declared style, and the layers
// buttons open over the screen - a menu, a dialog and a popover - and a tooltip. Each button emits an
// action the application handles; here the variants do nothing, and the rest open their layers. A
// right click on More, or a finger resting on it, opens its menu at the point pressed.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, Dialog, HeaderBar, Menu, Popover, Row, Screen, Spacer, Text, Tooltip } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
  onTry: () => void;
  onMore: () => void;
  onAsk: () => void;
  onInfo: () => void;
  onDone: () => void;
  menuOpen: boolean;
  asking: boolean;
  infoOpen: boolean;
};

export function GalleryButtonsScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Buttons
          </Text>
          <Spacer />
          <Text>
            4 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Button variant="primary" onPress={props.onTry}>
              Primary
            </Button>
            <Text role="caption">{'variant="primary"'}</Text>
          </Column>
          <Column gap={4}>
            <Button variant="secondary" onPress={props.onTry}>
              Secondary
            </Button>
            <Text role="caption">{'variant="secondary"'}</Text>
          </Column>
          <Column gap={4}>
            <Button variant="quiet" onPress={props.onTry}>
              Quiet
            </Button>
            <Text role="caption">{'variant="quiet"'}</Text>
          </Column>
        </Row>
        <Button onClick={() => props.onTry()} style={{ backgroundColor: "text", color: "background" }}>
          Styled
        </Button>
        <Text role="caption">{'<Button style={{ backgroundColor: "text", color: "background" }}>'}</Text>
        <Text role="caption">{'<Button onPress={props.onSave}>: a callback prop becomes an action'}</Text>
        <Text role="caption">{'<Button onClick={() => props.onSave()}>: React DOM\'s onClick, and a function that only calls a prop'}</Text>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Button id="more" variant="secondary" onPress={props.onMore} onContextMenu={props.onMore}>
              More
            </Button>
            <Menu for="more" open={props.menuOpen}>
              <Column>
                <Button variant="quiet" onPress={props.onTry}>
                  Rename
                </Button>
                <Button variant="quiet" onPress={props.onTry}>
                  Archive
                </Button>
              </Column>
            </Menu>
            <Text role="caption">{'<Menu for="more" open={...}>'}</Text>
          </Column>
          <Column gap={4}>
            <Button id="ask" onPress={props.onAsk}>
              Ask
            </Button>
            <Tooltip for="ask">Opens a dialog over the screen</Tooltip>
            <Text role="caption">{'<Dialog open={...}>, <Tooltip for="ask">'}</Text>
          </Column>
          <Column gap={4}>
            <Button id="info" variant="quiet" onPress={props.onInfo}>
              Info
            </Button>
            <Popover for="info" open={props.infoOpen}>
              <Text lines={2}>A popover shows beside its control, on the side with room.</Text>
            </Popover>
            <Text role="caption">{'<Popover for="info" open={...}>'}</Text>
          </Column>
        </Row>
        <Dialog open={props.asking}>
          <Column gap={GAP}>
            <Text role="heading">A dialog</Text>
            <Text lines={2}>The screen beneath is veiled and out of reach until it closes.</Text>
            <Button onPress={props.onDone}>Done</Button>
          </Column>
        </Dialog>
      </Column>
    </Screen>
  );
}
